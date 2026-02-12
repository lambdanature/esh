use std::iter::Peekable;
use std::str::Chars;

/// Errors that can occur when parsing a shell line.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShellParseError {
    #[error("unmatched single quote")]
    UnmatchedSingleQuote,
    #[error("unmatched double quote")]
    UnmatchedDoubleQuote,
    #[error("trailing backslash")]
    TrailingBackslash,
    #[error("invalid \\x hex escape sequence")]
    InvalidHexEscape,
    #[error("invalid \\u{{}} unicode escape sequence")]
    InvalidUnicodeEscape,
    #[error("invalid unicode code point: U+{0:04X}")]
    InvalidUnicodeCodePoint(u32),
    #[error("invalid UTF-8 in argument")]
    InvalidUtf8,
}

/// Parse a single string using double-quote escape rules.
///
/// Processes backslash escape sequences in `input` and returns the raw bytes.
/// This is useful for interpreting escape sequences in individual values, such
/// as arguments obtained from [`std::env::args`].
///
/// Unlike [`shell_split`], double quotes are **not** treated as delimiters —
/// the entire input is consumed and `"` characters are kept literally.
/// Unknown `\X` sequences are preserved as `\X` (double-quote semantics).
///
/// The result is a byte vector because `\xNN` escapes produce raw bytes that
/// may not be valid UTF-8.
///
/// See [`shell_split`] for the full list of supported escape sequences.
///
/// # Errors
///
/// Returns [`ShellParseError`] on trailing backslash or malformed escape
/// sequences.
///
/// # Examples
///
/// ```
/// # use clap_sh::{parse_double_quoted, ShellParseError};
/// assert_eq!(parse_double_quoted(r"hello\nworld")?, b"hello\nworld");
/// assert_eq!(parse_double_quoted(r"\x41\x42\x43")?, b"ABC");
/// assert_eq!(parse_double_quoted(r"\xFF")?, b"\xff");
/// assert_eq!(parse_double_quoted(r"\u{1f980}")?, "🦀".as_bytes());
/// # Ok::<(), ShellParseError>(())
/// ```
pub fn parse_double_quoted(input: &str) -> Result<Vec<u8>, ShellParseError> {
    let mut chars = input.chars().peekable();
    let mut output = Vec::new();
    while let Some(c) = chars.next() {
        match c {
            '\\' => parse_backslash_escape(&mut chars, &mut output, true)?,
            _ => push_char(&mut output, c),
        }
    }
    Ok(output)
}

/// Inner double-quote parser that operates on a char iterator.
///
/// Appends parsed content to `output`.  Returns `true` if terminated by a
/// closing `"`, or `false` if the iterator was exhausted.
fn parse_double_quoted_inner(
    chars: &mut Peekable<Chars>,
    output: &mut Vec<u8>,
) -> Result<bool, ShellParseError> {
    while let Some(c) = chars.next() {
        match c {
            '"' => return Ok(true),
            '\\' => parse_backslash_escape(chars, output, true)?,
            _ => push_char(output, c),
        }
    }
    Ok(false)
}

/// Split a string into words using POSIX shell-like parsing rules.
///
/// Supports:
/// - **Unquoted words** split on whitespace
/// - **Single quotes** (`'...'`): everything inside is literal, no escape processing
/// - **Double quotes** (`"..."`): allows escape sequences; unknown `\X` is kept as `\X`
/// - **Backslash escapes** (in unquoted and double-quoted contexts):
///   - `\\`, `\'`, `\"`, `\$`, `` \` ``, `\ ` (literal versions)
///   - `\a` (bell), `\b` (backspace), `\e`/`\E` (escape 0x1B), `\f` (form feed),
///     `\n` (newline), `\r` (carriage return), `\t` (tab), `\v` (vertical tab)
///   - `\0[ooo]` — octal (up to 3 octal digits after the `0`)
///   - `\x[HH]` — C-style hex byte (1–2 hex digits)
///   - `\u{H..H}` — Rust-style unicode scalar (1–6 hex digits inside braces)
/// - **`\` + newline** is a line continuation (both characters are discarded)
/// - **`#` comments** — an unquoted `#` at word start consumes the rest of the line
///
/// # Errors
///
/// Returns [`ShellParseError`] on unmatched quotes, trailing backslash,
/// malformed escape sequences, or if a resulting word is not valid UTF-8
/// (e.g. a bare `\xFF`).
///
/// # Examples
///
/// ```
/// # use clap_sh::{shell_split, ShellParseError};
/// let args = shell_split(r#"hello "world 'foo'" bar"#)?;
/// assert_eq!(args, vec!["hello", "world 'foo'", "bar"]);
///
/// let args = shell_split(r"one\ two three")?;
/// assert_eq!(args, vec!["one two", "three"]);
///
/// let args = shell_split(r"\x41\x42\x43")?;
/// assert_eq!(args, vec!["ABC"]);
///
/// let args = shell_split(r"\u{1f980}")?;
/// assert_eq!(args, vec!["🦀"]);
/// # Ok::<(), ShellParseError>(())
/// ```
pub fn shell_split(input: &str) -> Result<Vec<String>, ShellParseError> {
    let mut words: Vec<String> = Vec::new();
    let mut current: Vec<u8> = Vec::new();
    let mut in_word = false;
    let mut chars = input.chars().peekable();

    enum State {
        Normal,
        SingleQuoted,
    }

    let mut state = State::Normal;

    while let Some(c) = chars.next() {
        match state {
            State::Normal => match c {
                ' ' | '\t' | '\n' | '\r' => {
                    if in_word {
                        let s = String::from_utf8(std::mem::take(&mut current))
                            .map_err(|_| ShellParseError::InvalidUtf8)?;
                        words.push(s);
                        in_word = false;
                    }
                }
                '\'' => {
                    in_word = true;
                    state = State::SingleQuoted;
                }
                '"' => {
                    in_word = true;
                    if !parse_double_quoted_inner(&mut chars, &mut current)? {
                        return Err(ShellParseError::UnmatchedDoubleQuote);
                    }
                }
                '\\' => {
                    in_word = true;
                    parse_backslash_escape(&mut chars, &mut current, false)?;
                }
                '#' if !in_word => {
                    // Comment — consume the rest of the input
                    break;
                }
                _ => {
                    in_word = true;
                    push_char(&mut current, c);
                }
            },
            State::SingleQuoted => match c {
                '\'' => {
                    state = State::Normal;
                }
                _ => {
                    push_char(&mut current, c);
                }
            },
        }
    }

    if matches!(state, State::SingleQuoted) {
        return Err(ShellParseError::UnmatchedSingleQuote);
    }

    if in_word {
        let s = String::from_utf8(current).map_err(|_| ShellParseError::InvalidUtf8)?;
        words.push(s);
    }

    Ok(words)
}

/// Append the UTF-8 encoding of `c` to a byte buffer.
fn push_char(output: &mut Vec<u8>, c: char) {
    let mut buf = [0u8; 4];
    let encoded = c.encode_utf8(&mut buf);
    output.extend_from_slice(encoded.as_bytes());
}

/// Convert an ASCII hex digit to its numeric value (0–15), or `None` if
/// the character is not a hex digit.
fn hex_digit(c: char) -> Option<u8> {
    match c {
        '0'..='9' => Some((c as u8) - b'0'),
        'a'..='f' => Some((c as u8) - b'a' + 10),
        'A'..='F' => Some((c as u8) - b'A' + 10),
        _ => None,
    }
}

/// Parse a backslash escape sequence, consuming characters from `chars` and
/// appending the result to `output`.
///
/// When `in_double_quotes` is true, an unrecognised `\X` is preserved as the
/// two characters `\X` (POSIX double-quote semantics).  When false (unquoted),
/// an unrecognised `\X` produces just `X` (POSIX unquoted semantics).
fn parse_backslash_escape(
    chars: &mut Peekable<Chars>,
    output: &mut Vec<u8>,
    in_double_quotes: bool,
) -> Result<(), ShellParseError> {
    let next = chars.next().ok_or(ShellParseError::TrailingBackslash)?;

    match next {
        // ---- simple escapes ------------------------------------------------
        'a' => output.push(0x07),
        'b' => output.push(0x08),
        'e' | 'E' => output.push(0x1B),
        'f' => output.push(0x0C),
        'n' => output.push(b'\n'),
        'r' => output.push(b'\r'),
        't' => output.push(b'\t'),
        'v' => output.push(0x0B),
        '\\' => output.push(b'\\'),
        '\'' => output.push(b'\''),
        '"' => output.push(b'"'),
        '$' => output.push(b'$'),
        '`' => output.push(b'`'),
        ' ' => output.push(b' '),

        // ---- line continuation ---------------------------------------------
        '\n' => { /* discard both backslash and newline */ }

        // ---- octal: \0[ooo] -----------------------------------------------
        '0' => {
            let mut value: u8 = 0;
            let mut count = 0u8;
            while count < 3 {
                match chars.peek() {
                    Some(&d) if d >= '0' && d <= '7' => {
                        value = value * 8 + (d as u8 - b'0');
                        chars.next();
                        count += 1;
                    }
                    _ => break,
                }
            }
            output.push(value);
        }

        // ---- C-style hex: \xH[H] ------------------------------------------
        'x' => {
            let mut value: u8 = 0;
            let mut count = 0u8;
            for _ in 0..2 {
                if let Some(h) = chars.peek().and_then(|&c| hex_digit(c)) {
                    value = (value << 4) | h;
                    chars.next();
                    count += 1;
                } else {
                    break;
                }
            }
            if count == 0 {
                return Err(ShellParseError::InvalidHexEscape);
            }
            output.push(value);
        }

        // ---- Rust-style unicode: \u{H..H} ---------------------------------
        'u' => {
            if chars.peek() != Some(&'{') {
                return Err(ShellParseError::InvalidUnicodeEscape);
            }
            chars.next(); // consume '{'

            let mut value: u32 = 0;
            let mut count = 0u8;
            loop {
                match chars.next() {
                    Some('}') => break,
                    Some(d) => {
                        let h = hex_digit(d).ok_or(ShellParseError::InvalidUnicodeEscape)?;
                        count += 1;
                        if count > 6 {
                            return Err(ShellParseError::InvalidUnicodeEscape);
                        }
                        value = (value << 4) | h as u32;
                    }
                    None => return Err(ShellParseError::InvalidUnicodeEscape),
                }
            }
            if count == 0 {
                return Err(ShellParseError::InvalidUnicodeEscape);
            }
            let ch =
                char::from_u32(value).ok_or(ShellParseError::InvalidUnicodeCodePoint(value))?;
            push_char(output, ch);
        }

        // ---- fallback ------------------------------------------------------
        other => {
            if in_double_quotes {
                // POSIX: in double quotes, unknown \X is kept literally as \X
                output.push(b'\\');
                push_char(output, other);
            } else {
                // POSIX: in unquoted context, \ quotes the next character
                push_char(output, other);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- basic splitting ---------------------------------------------------

    #[test]
    fn empty_input() {
        assert_eq!(shell_split("").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn whitespace_only() {
        assert_eq!(shell_split("   \t\n  ").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn simple_words() {
        assert_eq!(
            shell_split("hello world foo").unwrap(),
            vec!["hello", "world", "foo"],
        );
    }

    #[test]
    fn extra_whitespace() {
        assert_eq!(
            shell_split("  hello   world  ").unwrap(),
            vec!["hello", "world"],
        );
    }

    // ---- single quotes -----------------------------------------------------

    #[test]
    fn single_quoted() {
        assert_eq!(
            shell_split("'hello world' foo").unwrap(),
            vec!["hello world", "foo"],
        );
    }

    #[test]
    fn single_quoted_preserves_backslash() {
        assert_eq!(
            shell_split(r"'hello\nworld'").unwrap(),
            vec![r"hello\nworld"]
        );
    }

    #[test]
    fn empty_single_quotes() {
        assert_eq!(shell_split("'' foo").unwrap(), vec!["", "foo"]);
    }

    #[test]
    fn unmatched_single_quote() {
        assert_eq!(
            shell_split("'hello"),
            Err(ShellParseError::UnmatchedSingleQuote),
        );
    }

    // ---- double quotes -----------------------------------------------------

    #[test]
    fn double_quoted() {
        assert_eq!(
            shell_split(r#""hello world" foo"#).unwrap(),
            vec!["hello world", "foo"],
        );
    }

    #[test]
    fn double_quoted_escapes() {
        assert_eq!(
            shell_split(r#""hello\nworld""#).unwrap(),
            vec!["hello\nworld"],
        );
    }

    #[test]
    fn double_quoted_unknown_escape_preserved() {
        // \z is not a known escape, so in double quotes it stays as \z
        assert_eq!(shell_split(r#""\z""#).unwrap(), vec![r"\z"]);
    }

    #[test]
    fn empty_double_quotes() {
        assert_eq!(shell_split(r#""""#).unwrap(), vec![""]);
    }

    #[test]
    fn unmatched_double_quote() {
        assert_eq!(
            shell_split(r#""hello"#),
            Err(ShellParseError::UnmatchedDoubleQuote),
        );
    }

    // ---- unquoted backslash ------------------------------------------------

    #[test]
    fn backslash_space() {
        assert_eq!(shell_split(r"hello\ world").unwrap(), vec!["hello world"]);
    }

    #[test]
    fn backslash_newline_continuation() {
        assert_eq!(shell_split("hello\\\nworld").unwrap(), vec!["helloworld"]);
    }

    #[test]
    fn trailing_backslash() {
        assert_eq!(
            shell_split("hello\\"),
            Err(ShellParseError::TrailingBackslash),
        );
    }

    #[test]
    fn unquoted_unknown_escape_strips_backslash() {
        // In unquoted context, \z becomes z
        assert_eq!(shell_split(r"\z").unwrap(), vec!["z"]);
    }

    // ---- escape sequences --------------------------------------------------

    #[test]
    fn simple_escapes() {
        assert_eq!(shell_split(r"\a").unwrap(), vec!["\x07"]);
        assert_eq!(shell_split(r"\b").unwrap(), vec!["\x08"]);
        assert_eq!(shell_split(r"\e").unwrap(), vec!["\x1B"]);
        assert_eq!(shell_split(r"\E").unwrap(), vec!["\x1B"]);
        assert_eq!(shell_split(r"\f").unwrap(), vec!["\x0C"]);
        assert_eq!(shell_split(r"\n").unwrap(), vec!["\n"]);
        assert_eq!(shell_split(r"\r").unwrap(), vec!["\r"]);
        assert_eq!(shell_split(r"\t").unwrap(), vec!["\t"]);
        assert_eq!(shell_split(r"\v").unwrap(), vec!["\x0B"]);
        assert_eq!(shell_split(r"\\").unwrap(), vec!["\\"]);
        assert_eq!(shell_split(r"\'").unwrap(), vec!["'"]);
        assert_eq!(shell_split(r#"\""#).unwrap(), vec!["\""]);
    }

    #[test]
    fn octal_escape() {
        // \0101 = 'A' (65 decimal)
        assert_eq!(shell_split(r"\0101").unwrap(), vec!["A"]);
    }

    #[test]
    fn octal_nul() {
        assert_eq!(shell_split(r"\0").unwrap(), vec!["\0"]);
    }

    // ---- hex escape --------------------------------------------------------

    #[test]
    fn hex_escape() {
        assert_eq!(shell_split(r"\x41\x42\x43").unwrap(), vec!["ABC"]);
    }

    #[test]
    fn hex_escape_single_digit() {
        assert_eq!(shell_split(r"\xA").unwrap(), vec!["\n"]); // 0x0A = newline
    }

    #[test]
    fn hex_escape_invalid() {
        assert_eq!(shell_split(r"\xZZ"), Err(ShellParseError::InvalidHexEscape),);
    }

    #[test]
    fn hex_escape_high_byte_is_invalid_utf8_in_split() {
        // \xFF alone is not valid UTF-8, so shell_split rejects it
        assert_eq!(shell_split(r"\xFF"), Err(ShellParseError::InvalidUtf8));
    }

    // ---- hex escape via parse_double_quoted --------------------------------

    #[test]
    fn dq_hex_raw_byte() {
        // parse_double_quoted returns raw bytes — \xFF is byte 0xFF
        assert_eq!(parse_double_quoted(r"\xFF").unwrap(), vec![0xFF]);
    }

    #[test]
    fn dq_hex_high_bytes() {
        assert_eq!(
            parse_double_quoted(r"\x80\xFE\xFF").unwrap(),
            vec![0x80, 0xFE, 0xFF],
        );
    }

    // ---- unicode escape ----------------------------------------------------

    #[test]
    fn unicode_escape_ascii() {
        assert_eq!(shell_split(r"\u{41}").unwrap(), vec!["A"]);
    }

    #[test]
    fn unicode_escape_emoji() {
        assert_eq!(shell_split(r"\u{1f980}").unwrap(), vec!["🦀"]);
    }

    #[test]
    fn unicode_escape_missing_brace() {
        assert_eq!(
            shell_split(r"\u0041"),
            Err(ShellParseError::InvalidUnicodeEscape),
        );
    }

    #[test]
    fn unicode_escape_empty_braces() {
        assert_eq!(
            shell_split(r"\u{}"),
            Err(ShellParseError::InvalidUnicodeEscape),
        );
    }

    #[test]
    fn unicode_escape_too_many_digits() {
        assert_eq!(
            shell_split(r"\u{1234567}"),
            Err(ShellParseError::InvalidUnicodeEscape),
        );
    }

    #[test]
    fn unicode_escape_invalid_code_point() {
        assert_eq!(
            shell_split(r"\u{D800}"),
            Err(ShellParseError::InvalidUnicodeCodePoint(0xD800)),
        );
    }

    // ---- comments ----------------------------------------------------------

    #[test]
    fn comment_at_start() {
        assert_eq!(
            shell_split("# this is a comment").unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn comment_after_words() {
        assert_eq!(
            shell_split("hello world # comment").unwrap(),
            vec!["hello", "world"],
        );
    }

    #[test]
    fn hash_inside_word_is_not_comment() {
        assert_eq!(shell_split("foo#bar").unwrap(), vec!["foo#bar"]);
    }

    #[test]
    fn hash_in_quotes_is_not_comment() {
        assert_eq!(
            shell_split(r##""# not a comment""##).unwrap(),
            vec!["# not a comment"]
        );
    }

    // ---- parse_double_quoted ------------------------------------------------

    #[test]
    fn dq_parse_plain() {
        assert_eq!(parse_double_quoted("hello world").unwrap(), b"hello world");
    }

    #[test]
    fn dq_parse_escapes() {
        assert_eq!(
            parse_double_quoted(r"hello\nworld").unwrap(),
            b"hello\nworld"
        );
    }

    #[test]
    fn dq_parse_hex() {
        assert_eq!(parse_double_quoted(r"\x41\x42\x43").unwrap(), b"ABC");
    }

    #[test]
    fn dq_parse_unicode() {
        assert_eq!(parse_double_quoted(r"\u{1f980}").unwrap(), "🦀".as_bytes());
    }

    #[test]
    fn dq_parse_quotes_are_literal() {
        assert_eq!(
            parse_double_quoted(r#"hello "world""#).unwrap(),
            br#"hello "world""#.to_vec(),
        );
    }

    #[test]
    fn dq_parse_unknown_escape_preserved() {
        assert_eq!(parse_double_quoted(r"\z").unwrap(), br"\z".to_vec());
    }

    #[test]
    fn dq_parse_empty() {
        assert_eq!(parse_double_quoted("").unwrap(), b"");
    }

    #[test]
    fn dq_parse_trailing_backslash() {
        assert_eq!(
            parse_double_quoted("hello\\"),
            Err(ShellParseError::TrailingBackslash),
        );
    }

    // ---- mixed quoting -----------------------------------------------------

    #[test]
    fn adjacent_quotes_merge() {
        assert_eq!(
            shell_split(r#"hel"lo wo"rld"#).unwrap(),
            vec!["hello world"]
        );
    }

    #[test]
    fn single_inside_double() {
        assert_eq!(
            shell_split(r#""it's a test""#).unwrap(),
            vec!["it's a test"],
        );
    }

    #[test]
    fn double_inside_single() {
        assert_eq!(
            shell_split(r#"'say "hello"'"#).unwrap(),
            vec![r#"say "hello""#],
        );
    }

    #[test]
    fn complex_mixed() {
        assert_eq!(
            shell_split(r#"echo "hello 'world'" foo\ bar 'baz "qux"'"#).unwrap(),
            vec!["echo", "hello 'world'", "foo bar", r#"baz "qux""#],
        );
    }
}
