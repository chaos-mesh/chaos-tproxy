use std::process::{Command, Output};

use super::BINARY;

fn run(args: &[&str]) -> Output {
    Command::new(BINARY).args(args).output().unwrap()
}

#[test]
fn help_succeeds_on_stdout() {
    for flag in ["--help", "-h"] {
        let output = run(&[flag]);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("USAGE:"), "{}", stdout);
        assert!(stdout.contains("--interactive"), "{}", stdout);
    }
}

#[test]
fn version_succeeds_on_stdout() {
    for flag in ["--version", "-V"] {
        let output = run(&[flag]);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, b"chaos-tproxy 0.5.6\n");
    }
}

#[test]
fn invalid_argument_fails_on_stderr() {
    let output = run(&["--invalid-option"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("Found argument '--invalid-option' which wasn't expected"),
        "{}",
        stderr
    );
}

#[test]
fn missing_configuration_fails_on_stderr() {
    let output = run(&[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "config file is required when interactive mode and daemon mode is all disabled, use `-h | --help` for more details\n"
    );
}
