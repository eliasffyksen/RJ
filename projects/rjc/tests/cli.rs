//! Runs the `rjc` binary the way tools and editors do.

use std::{path::PathBuf, process::Command};

fn rjc(args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_rjc")).args(args).output().unwrap();

    (
        output.status.success(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

fn source_file(name: &str, source: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, source).unwrap();
    path
}

#[test]
fn json_diagnostics() {
    let path = source_file(
        "mismatch.rj",
        "type NodeId i32\nfn main(): i32 {\n  id: NodeId\n  i: i32\n  i = 1\n  id = i\n  return 0\n}\n",
    );

    let (ok, stdout, _) = rjc(&["--json", path.to_str().unwrap()]);

    assert!(!ok);
    assert!(stdout.starts_with(r#"{"diagnostics":[{"code":"E0200""#), "{stdout}");
    assert!(stdout.contains(r#""line":6,"column":8"#), "{stdout}");
    assert!(stdout.contains(r#""help":"convert it: `NodeId(...)`""#), "{stdout}");
    assert!(stdout.contains(r#""expected":"NodeId","found":"i32""#), "{stdout}");
}

#[test]
fn json_for_a_correct_program() {
    let path = source_file("ok.rj", "fn main(): i32 { return 0 }\n");

    let (ok, stdout, _) = rjc(&["--json", path.to_str().unwrap()]);

    assert!(ok);
    assert_eq!(stdout.trim(), r#"{"diagnostics":[]}"#);
}

#[test]
fn json_for_a_syntax_error() {
    let path = source_file("syntax.rj", "fn main(): i32 { return 1 + }\n");

    let (ok, stdout, _) = rjc(&["--json", path.to_str().unwrap()]);

    assert!(!ok);
    assert!(stdout.contains(r#""code":"E0001""#), "{stdout}");
}

#[test]
fn human_readable_diagnostics_show_code_and_help() {
    let path = source_file("unknown.rj", "fn main(): i32 {\n  total: i32\n  total = 1\n  return totla\n}\n");

    let (ok, _, stderr) = rjc(&[path.to_str().unwrap()]);

    assert!(!ok);
    assert!(stderr.contains("error[E0100]: unknown variable `totla`"), "{stderr}");
    assert!(stderr.contains("help: did you mean `total`?"), "{stderr}");
}

#[test]
fn explain() {
    let (ok, stdout, _) = rjc(&["--explain", "E0300"]);
    assert!(ok);
    assert!(stdout.starts_with("E0300: used before assigned"), "{stdout}");

    let (ok, _, stderr) = rjc(&["--explain", "E9999"]);
    assert!(!ok);
    assert!(stderr.contains("unknown error code"), "{stderr}");
}
