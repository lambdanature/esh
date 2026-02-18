use assert_cmd::Command;
use predicates::prelude::*;

fn esh() -> Command {
    Command::from(assert_cmd::cargo::cargo_bin_cmd!("esh"))
}

// -- version ---------------------------------------------------------------

#[test]
fn version_outputs_package_info() {
    esh()
        .args(["-p", "/tmp", "version"])
        .assert()
        .success()
        .stdout(predicate::str::contains("esh"))
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

// -- pwd -------------------------------------------------------------------

#[test]
fn pwd_with_tmp() {
    esh()
        .args(["-p", "/tmp", "pwd"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn pwd_with_tempdir() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    esh()
        .args(["-p", dir.path().to_str().unwrap(), "pwd"])
        .assert()
        .success()
        .stdout(predicate::str::contains("/"));
}

// -- error cases -----------------------------------------------------------

#[test]
fn nonexistent_path_fails() {
    esh()
        .args(["-p", "/nonexistent", "pwd"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("cannot open path"));
}

#[test]
fn path_pointing_to_file_fails() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let file_path = dir.path().join("not-a-dir");
    std::fs::write(&file_path, "").expect("failed to create file");
    esh()
        .args(["-p", file_path.to_str().unwrap(), "pwd"])
        .assert()
        .failure()
        .code(2)
        .stderr(
            predicate::str::contains("not a directory")
                .or(predicate::str::contains("cannot open path")),
        );
}

#[test]
fn unknown_subcommand_fails_with_usage() {
    esh()
        .arg("nosuchcmd")
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn no_args_shows_help() {
    esh()
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("Usage").and(predicate::str::contains("COMMAND")));
}

// -- flags -----------------------------------------------------------------

#[test]
fn quiet_flag_accepted() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    esh()
        .args(["-p", dir.path().to_str().unwrap(), "-q", "version"])
        .assert()
        .success();
}

#[test]
fn verbose_flag_accepted() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    esh()
        .args(["-p", dir.path().to_str().unwrap(), "-v", "version"])
        .assert()
        .success();
}

#[test]
fn multiple_verbose_flags_accepted() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    esh()
        .args(["-p", dir.path().to_str().unwrap(), "-vvv", "version"])
        .assert()
        .success();
}

#[test]
fn help_flag_shows_help() {
    esh().arg("--help").assert().success().stdout(
        predicate::str::contains("Usage")
            .and(predicate::str::contains("Options"))
            .and(predicate::str::contains("Commands")),
    );
}

#[test]
fn version_flag_shows_version() {
    esh()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

// -- shell subcommand ------------------------------------------------------

#[test]
fn shell_subcommand_exits_with_error() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    esh()
        .args(["-p", dir.path().to_str().unwrap(), "shell"])
        .assert()
        .failure();
}

// -- combined flags and commands -------------------------------------------

#[test]
fn quiet_and_verbose_together() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    esh()
        .args(["-p", dir.path().to_str().unwrap(), "-q", "-v", "version"])
        .assert()
        .success();
}

#[test]
fn flags_after_subcommand() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    esh()
        .args(["-p", dir.path().to_str().unwrap(), "version", "-q"])
        .assert()
        .success();
}
