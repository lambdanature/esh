use std::ffi::OsString;
use std::iter::Peekable;
use std::str::Chars;

use os_str_bytes::OsStringBytes;

/// Errors that can occur when parsing a shell line.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShellParseError {
    /// A single-quoted string was never closed.
    #[error("unmatched single quote")]
    UnmatchedSingleQuote,
    /// A double-quoted string was never closed.
    #[error("unmatched double quote")]
    UnmatchedDoubleQuote,
    /// Input ends with a lone backslash.
    #[error("trailing backslash")]
    TrailingBackslash,
    /// A `\xNN` sequence is malformed or incomplete.
    #[error("invalid \\x hex escape sequence")]
    InvalidHexEscape,
    /// A `\u{NNNN}` sequence is malformed or incomplete.
    #[error("invalid \\u{{}} unicode escape sequence")]
    InvalidUnicodeEscape,
    /// The code point in a `\u{NNNN}` escape is not a valid Unicode scalar value.
    #[error("invalid unicode code point: U+{0:04X}")]
    InvalidUnicodeCodePoint(u32),
    /// The resulting byte sequence is not valid UTF-8.
    #[error("invalid UTF-8 in argument")]
    InvalidUtf8,
}

/// Parse a single string using double-quote escape rules, returning an
/// [`OsString`].
///
/// This is a thin wrapper around [`shell_parse_arg_bytes`] — see that
/// function for full documentation on parsing behaviour and supported
/// escape sequences.
///
/// # Errors
///
/// Returns [`ShellParseError`] on malformed input.  On Windows, also
/// returns [`ShellParseError::InvalidUtf8`] when the resulting bytes
/// cannot be represented as an `OsString` (e.g. `\xFF`).
///
/// # Examples
///
/// ```
/// # use esh::{shell_parse_arg, ShellParseError};
/// assert_eq!(shell_parse_arg(r"hello\nworld")?, "hello\nworld");
/// assert_eq!(shell_parse_arg(r"\x41\x42\x43")?, "ABC");
/// assert_eq!(shell_parse_arg(r"\u{1f980}")?, "🦀");
/// # Ok::<(), ShellParseError>(())
/// ```
/// Convenience wrapper around [`shell_parse_arg_bytes`] that converts the
/// result to an [`OsString`].
///
/// On Windows, returns [`ShellParseError::InvalidUtf8`] if the byte sequence
/// cannot be represented as an `OsString` (e.g. a bare `\xFF`).
/// Use [`shell_parse_arg_bytes`] when you need the raw bytes on all platforms.
pub fn shell_parse_arg(input: &str) -> Result<OsString, ShellParseError> {
    let bytes = shell_parse_arg_bytes(input)?;
    OsString::from_io_vec(bytes).ok_or(ShellParseError::InvalidUtf8)
}

/// Parse a single string using double-quote escape rules, returning raw bytes.
///
/// This is the primary byte-level parser. It processes backslash escape
/// sequences in `input` and returns the raw bytes, which may include
/// non-UTF-8 values such as `\xFF`.
///
/// Unlike [`shell_parse_line_bytes`], double quotes are **not** treated as
/// delimiters — the entire input is consumed and `"` characters are kept
/// literally. Unknown `\X` sequences are preserved as `\X` (double-quote
/// semantics).
///
/// See [`shell_parse_line`] for the full list of supported escape sequences.
///
/// # Errors
///
/// Returns [`ShellParseError`] on trailing backslash or malformed escape
/// sequences.
///
/// # Examples
///
/// ```
/// # use esh::{shell_parse_arg_bytes, ShellParseError};
/// assert_eq!(shell_parse_arg_bytes(r"hello\nworld")?, b"hello\nworld");
/// assert_eq!(shell_parse_arg_bytes(r"\x41\x42\x43")?, b"ABC");
/// assert_eq!(shell_parse_arg_bytes(r"\xFF")?, vec![0xFF]);
/// # Ok::<(), ShellParseError>(())
/// ```
pub fn shell_parse_arg_bytes(input: &str) -> Result<Vec<u8>, ShellParseError> {
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
fn shell_parse_arg_inner(
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

/// Split a string into words using POSIX shell-like parsing rules, returning
/// [`OsString`] values.
///
/// This is a thin wrapper around [`shell_parse_line_bytes`] — see that
/// function for full documentation on parsing behaviour and supported
/// escape sequences.
///
/// # Errors
///
/// Returns [`ShellParseError`] on malformed input.  On Windows, also
/// returns [`ShellParseError::InvalidUtf8`] when a resulting word contains
/// bytes that cannot be represented as an `OsString` (e.g. `\xFF`).
///
/// # Examples
///
/// ```
/// # use esh::{shell_parse_line, ShellParseError};
/// let args = shell_parse_line(r#"hello "world 'foo'" bar"#)?;
/// assert_eq!(args, vec!["hello", "world 'foo'", "bar"]);
///
/// let args = shell_parse_line(r"one\ two three")?;
/// assert_eq!(args, vec!["one two", "three"]);
/// # Ok::<(), ShellParseError>(())
/// ```
pub fn shell_parse_line(input: &str) -> Result<Vec<OsString>, ShellParseError> {
    shell_parse_line_bytes(input)?
        .into_iter()
        .map(|w| OsString::from_io_vec(w).ok_or(ShellParseError::InvalidUtf8))
        .collect()
}

/// Split a string into words using POSIX shell-like parsing rules, returning
/// raw byte vectors.
///
/// This is the primary byte-level word splitter. Each word is returned as a
/// `Vec<u8>` that may contain non-UTF-8 bytes (e.g. from `\xFF` escapes).
///
/// ## Parsing rules
///
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
/// Returns [`ShellParseError`] on unmatched quotes, trailing backslash, or
/// malformed escape sequences.
///
/// # Examples
///
/// ```
/// # use esh::{shell_parse_line_bytes, ShellParseError};
/// let words = shell_parse_line_bytes(r"\x41\x42\x43")?;
/// assert_eq!(words, vec![b"ABC".to_vec()]);
///
/// let words = shell_parse_line_bytes(r"\xFF")?;
/// assert_eq!(words, vec![vec![0xFF]]);
/// # Ok::<(), ShellParseError>(())
/// ```
pub fn shell_parse_line_bytes(input: &str) -> Result<Vec<Vec<u8>>, ShellParseError> {
    enum State {
        Normal,
        SingleQuoted,
    }

    let mut words: Vec<Vec<u8>> = Vec::new();
    let mut current: Vec<u8> = Vec::new();
    let mut in_word = false;
    let mut chars = input.chars().peekable();
    let mut state = State::Normal;

    while let Some(c) = chars.next() {
        match state {
            State::Normal => match c {
                ' ' | '\t' | '\n' | '\r' => {
                    if in_word {
                        words.push(std::mem::take(&mut current));
                        in_word = false;
                    }
                }
                '\'' => {
                    in_word = true;
                    state = State::SingleQuoted;
                }
                '"' => {
                    in_word = true;
                    if !shell_parse_arg_inner(&mut chars, &mut current)? {
                        return Err(ShellParseError::UnmatchedDoubleQuote);
                    }
                }
                '\\' => {
                    in_word = true;
                    parse_backslash_escape(&mut chars, &mut current, false)?;
                }
                '#' if !in_word => {
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
        words.push(current);
    }

    Ok(words)
}

/// Append the UTF-8 encoding of `c` to a byte buffer.
#[inline]
fn push_char(output: &mut Vec<u8>, c: char) {
    let mut buf = [0u8; 4];
    let encoded = c.encode_utf8(&mut buf);
    output.extend_from_slice(encoded.as_bytes());
}

/// Convert an ASCII hex digit to its numeric value (0–15), or `None` if
/// the character is not a hex digit.
#[inline]
const fn hex_digit(c: char) -> Option<u8> {
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
#[inline]
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
        // Capped at \0377 (255) like POSIX $'...' — digits that would
        // overflow a u8 are left unconsumed.
        '0' => {
            let mut value: u16 = 0;
            let mut count = 0u8;
            while count < 3 {
                match chars.peek() {
                    Some(&d) if ('0'..='7').contains(&d) => {
                        let next_value = value * 8 + (d as u16 - u16::from(b'0'));
                        if next_value > 255 {
                            break;
                        }
                        value = next_value;
                        chars.next();
                        count += 1;
                    }
                    _ => break,
                }
            }
            #[allow(clippy::cast_possible_truncation)] // guarded by the > 255 check above
            output.push(value as u8);
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
                        value = (value << 4) | u32::from(h);
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
            }
            // POSIX: in unquoted context, \ quotes the next character;
            // in double quotes, the backslash is already emitted above.
            push_char(output, other);
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
        assert_eq!(shell_parse_line("").unwrap(), Vec::<OsString>::new());
    }

    #[test]
    fn whitespace_only() {
        assert_eq!(
            shell_parse_line("   \t\n  ").unwrap(),
            Vec::<OsString>::new()
        );
    }

    #[test]
    fn simple_words() {
        assert_eq!(
            shell_parse_line("hello world foo").unwrap(),
            vec!["hello", "world", "foo"],
        );
    }

    #[test]
    fn extra_whitespace() {
        assert_eq!(
            shell_parse_line("  hello   world  ").unwrap(),
            vec!["hello", "world"],
        );
    }

    // ---- single quotes -----------------------------------------------------

    #[test]
    fn single_quoted() {
        assert_eq!(
            shell_parse_line("'hello world' foo").unwrap(),
            vec!["hello world", "foo"],
        );
    }

    #[test]
    fn single_quoted_preserves_backslash() {
        assert_eq!(
            shell_parse_line(r"'hello\nworld'").unwrap(),
            vec![r"hello\nworld"]
        );
    }

    #[test]
    fn empty_single_quotes() {
        assert_eq!(shell_parse_line("'' foo").unwrap(), vec!["", "foo"]);
    }

    #[test]
    fn unmatched_single_quote() {
        assert_eq!(
            shell_parse_line("'hello"),
            Err(ShellParseError::UnmatchedSingleQuote),
        );
    }

    // ---- double quotes -----------------------------------------------------

    #[test]
    fn double_quoted() {
        assert_eq!(
            shell_parse_line(r#""hello world" foo"#).unwrap(),
            vec!["hello world", "foo"],
        );
    }

    #[test]
    fn double_quoted_escapes() {
        assert_eq!(
            shell_parse_line(r#""hello\nworld""#).unwrap(),
            vec!["hello\nworld"],
        );
    }

    #[test]
    fn double_quoted_unknown_escape_preserved() {
        // \z is not a known escape, so in double quotes it stays as \z
        assert_eq!(shell_parse_line(r#""\z""#).unwrap(), vec![r"\z"]);
    }

    #[test]
    fn empty_double_quotes() {
        assert_eq!(shell_parse_line(r#""""#).unwrap(), vec![""]);
    }

    #[test]
    fn unmatched_double_quote() {
        assert_eq!(
            shell_parse_line(r#""hello"#),
            Err(ShellParseError::UnmatchedDoubleQuote),
        );
    }

    // ---- unquoted backslash ------------------------------------------------

    #[test]
    fn backslash_space() {
        assert_eq!(
            shell_parse_line(r"hello\ world").unwrap(),
            vec!["hello world"]
        );
    }

    #[test]
    fn backslash_newline_continuation() {
        assert_eq!(
            shell_parse_line("hello\\\nworld").unwrap(),
            vec!["helloworld"]
        );
    }

    #[test]
    fn trailing_backslash() {
        assert_eq!(
            shell_parse_line("hello\\"),
            Err(ShellParseError::TrailingBackslash),
        );
    }

    #[test]
    fn unquoted_unknown_escape_strips_backslash() {
        // In unquoted context, \z becomes z
        assert_eq!(shell_parse_line(r"\z").unwrap(), vec!["z"]);
    }

    // ---- escape sequences --------------------------------------------------

    #[test]
    fn simple_escapes() {
        assert_eq!(shell_parse_line(r"\a").unwrap(), vec!["\x07"]);
        assert_eq!(shell_parse_line(r"\b").unwrap(), vec!["\x08"]);
        assert_eq!(shell_parse_line(r"\e").unwrap(), vec!["\x1B"]);
        assert_eq!(shell_parse_line(r"\E").unwrap(), vec!["\x1B"]);
        assert_eq!(shell_parse_line(r"\f").unwrap(), vec!["\x0C"]);
        assert_eq!(shell_parse_line(r"\n").unwrap(), vec!["\n"]);
        assert_eq!(shell_parse_line(r"\r").unwrap(), vec!["\r"]);
        assert_eq!(shell_parse_line(r"\t").unwrap(), vec!["\t"]);
        assert_eq!(shell_parse_line(r"\v").unwrap(), vec!["\x0B"]);
        assert_eq!(shell_parse_line(r"\\").unwrap(), vec!["\\"]);
        assert_eq!(shell_parse_line(r"\'").unwrap(), vec!["'"]);
        assert_eq!(shell_parse_line(r#"\""#).unwrap(), vec!["\""]);
    }

    #[test]
    fn octal_escape() {
        // \0101 = 'A' (65 decimal)
        assert_eq!(shell_parse_line(r"\0101").unwrap(), vec!["A"]);
    }

    #[test]
    fn octal_max() {
        assert_eq!(shell_parse_line_bytes(r"\0377").unwrap(), vec![vec![0xFF]],);
    }

    #[test]
    fn octal_overflow_stops_early() {
        // \0777: first two digits give \077 = 63 = '?', third '7' would
        // push to 511 which overflows u8, so it stays as literal text.
        assert_eq!(shell_parse_line(r"\0777").unwrap(), vec!["?7"]);
    }

    #[test]
    fn octal_nul() {
        assert_eq!(shell_parse_line(r"\0").unwrap(), vec!["\0"]);
    }

    // ---- hex escape --------------------------------------------------------

    #[test]
    fn hex_escape() {
        assert_eq!(shell_parse_line(r"\x41\x42\x43").unwrap(), vec!["ABC"]);
    }

    #[test]
    fn hex_escape_single_digit() {
        assert_eq!(shell_parse_line(r"\xA").unwrap(), vec!["\n"]); // 0x0A = newline
    }

    #[test]
    fn hex_escape_invalid() {
        assert_eq!(
            shell_parse_line(r"\xZZ"),
            Err(ShellParseError::InvalidHexEscape),
        );
    }

    #[test]
    fn hex_escape_high_byte_in_split() {
        assert_eq!(shell_parse_line_bytes(r"\xFF").unwrap(), vec![vec![0xFF]],);
    }

    // ---- hex escape via shell_parse_arg --------------------------------

    #[test]
    fn dq_hex_raw_byte() {
        assert_eq!(shell_parse_arg_bytes(r"\xFF").unwrap(), vec![0xFF],);
    }

    #[test]
    fn dq_hex_high_bytes() {
        assert_eq!(
            shell_parse_arg_bytes(r"\x80\xFE\xFF").unwrap(),
            vec![0x80, 0xFE, 0xFF],
        );
    }

    // ---- unicode escape ----------------------------------------------------

    #[test]
    fn unicode_escape_ascii() {
        assert_eq!(shell_parse_line(r"\u{41}").unwrap(), vec!["A"]);
    }

    #[test]
    fn unicode_escape_emoji() {
        assert_eq!(shell_parse_line(r"\u{1f980}").unwrap(), vec!["🦀"]);
    }

    #[test]
    fn unicode_escape_missing_brace() {
        assert_eq!(
            shell_parse_line(r"\u0041"),
            Err(ShellParseError::InvalidUnicodeEscape),
        );
    }

    #[test]
    fn unicode_escape_empty_braces() {
        assert_eq!(
            shell_parse_line(r"\u{}"),
            Err(ShellParseError::InvalidUnicodeEscape),
        );
    }

    #[test]
    fn unicode_escape_too_many_digits() {
        assert_eq!(
            shell_parse_line(r"\u{1234567}"),
            Err(ShellParseError::InvalidUnicodeEscape),
        );
    }

    #[test]
    fn unicode_escape_invalid_code_point() {
        assert_eq!(
            shell_parse_line(r"\u{D800}"),
            Err(ShellParseError::InvalidUnicodeCodePoint(0xD800)),
        );
    }

    // ---- comments ----------------------------------------------------------

    #[test]
    fn comment_at_start() {
        assert_eq!(
            shell_parse_line("# this is a comment").unwrap(),
            Vec::<OsString>::new()
        );
    }

    #[test]
    fn comment_after_words() {
        assert_eq!(
            shell_parse_line("hello world # comment").unwrap(),
            vec!["hello", "world"],
        );
    }

    #[test]
    fn hash_inside_word_is_not_comment() {
        assert_eq!(shell_parse_line("foo#bar").unwrap(), vec!["foo#bar"]);
    }

    #[test]
    fn hash_in_quotes_is_not_comment() {
        assert_eq!(
            shell_parse_line(r##""# not a comment""##).unwrap(),
            vec!["# not a comment"]
        );
    }

    // ---- shell_parse_arg ------------------------------------------------

    #[test]
    fn dq_parse_plain() {
        assert_eq!(shell_parse_arg("hello world").unwrap(), "hello world");
    }

    #[test]
    fn dq_parse_escapes() {
        assert_eq!(shell_parse_arg(r"hello\nworld").unwrap(), "hello\nworld");
    }

    #[test]
    fn dq_parse_hex() {
        assert_eq!(shell_parse_arg(r"\x41\x42\x43").unwrap(), "ABC");
    }

    #[test]
    fn dq_parse_unicode() {
        assert_eq!(shell_parse_arg(r"\u{1f980}").unwrap(), "🦀");
    }

    #[test]
    fn dq_parse_quotes_are_literal() {
        assert_eq!(
            shell_parse_arg(r#"hello "world""#).unwrap(),
            r#"hello "world""#,
        );
    }

    #[test]
    fn dq_parse_unknown_escape_preserved() {
        assert_eq!(shell_parse_arg(r"\z").unwrap(), r"\z");
    }

    #[test]
    fn dq_parse_empty() {
        assert_eq!(shell_parse_arg("").unwrap(), "");
    }

    #[test]
    fn dq_parse_trailing_backslash() {
        assert_eq!(
            shell_parse_arg("hello\\"),
            Err(ShellParseError::TrailingBackslash),
        );
    }

    // ---- mixed quoting -----------------------------------------------------

    #[test]
    fn adjacent_quotes_merge() {
        assert_eq!(
            shell_parse_line(r#"hel"lo wo"rld"#).unwrap(),
            vec!["hello world"]
        );
    }

    #[test]
    fn single_inside_double() {
        assert_eq!(
            shell_parse_line(r#""it's a test""#).unwrap(),
            vec!["it's a test"],
        );
    }

    #[test]
    fn double_inside_single() {
        assert_eq!(
            shell_parse_line(r#"'say "hello"'"#).unwrap(),
            vec![r#"say "hello""#],
        );
    }

    #[test]
    fn complex_mixed() {
        assert_eq!(
            shell_parse_line(r#"echo "hello 'world'" foo\ bar 'baz "qux"'"#).unwrap(),
            vec!["echo", "hello 'world'", "foo bar", r#"baz "qux""#],
        );
    }

    #[test]
    fn deeply_nested_quoting() {
        // "a'b\"c'd"e — double-quoted region containing singles and escaped double,
        // then unquoted text appended to the same word
        assert_eq!(
            shell_parse_line(r#""a'b\"c'd"e"#).unwrap(),
            vec!["a'b\"c'de"],
        );
    }

    #[test]
    fn shell_parse_arg_empty_input() {
        assert_eq!(shell_parse_arg("").unwrap(), OsString::from(""));
    }

    #[test]
    fn shell_parse_arg_only_escapes() {
        assert_eq!(shell_parse_arg(r"\n\t\r").unwrap(), "\n\t\r");
    }

    #[test]
    fn max_length_octal() {
        assert_eq!(shell_parse_arg_bytes(r"\0377").unwrap(), vec![0xFF],);
    }

    #[test]
    fn max_length_hex() {
        assert_eq!(shell_parse_arg_bytes(r"\xFF").unwrap(), vec![0xFF],);
    }

    #[test]
    fn max_length_unicode() {
        // \u{10FFFF} is the maximum valid Unicode code point
        assert_eq!(shell_parse_line(r"\u{10FFFF}").unwrap(), vec!["\u{10FFFF}"],);
    }

    #[test]
    fn octal_overflow_all_digits() {
        // \0400 would be 256, which overflows u8. Only \040 (32, space) is
        // consumed; the trailing '0' is literal.
        assert_eq!(shell_parse_line(r"\0400").unwrap(), vec![" 0"]);
    }

    #[test]
    fn multiline_continuation() {
        assert_eq!(
            shell_parse_line("hello\\\nworld").unwrap(),
            vec!["helloworld"],
        );
    }

    #[test]
    fn multiline_continuation_with_whitespace() {
        assert_eq!(
            shell_parse_line("one\\\n  two three").unwrap(),
            vec!["one", "two", "three"],
        );
    }

    #[test]
    fn long_input_string() {
        let long_word = "a".repeat(100_000);
        let result = shell_parse_line(&long_word).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 100_000);
    }

    #[test]
    fn long_input_many_words() {
        let input = "word ".repeat(10_000);
        let result = shell_parse_line(input.trim_end()).unwrap();
        assert_eq!(result.len(), 10_000);
    }
}
