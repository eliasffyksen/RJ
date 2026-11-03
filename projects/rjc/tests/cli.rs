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

#[test]
fn failing_example_in_json() {
    let path = source_file(
        "example.rj",
        "fn add(a: i32, b: i32): i32 { return a - b }\nexample add(2, 3) == 5\nfn main(): i32 { return 0 }\n",
    );

    let (ok, stdout, _) = rjc(&["--json", path.to_str().unwrap()]);

    assert!(!ok);
    assert!(stdout.contains(r#""code":"E0500""#), "{stdout}");
    assert!(stdout.contains(r#""line":2"#), "{stdout}");
    assert!(stdout.contains(r#""expected":"5","found":"-1""#), "{stdout}");
}

#[test]
fn copy_notes_in_json() {
    let path = source_file(
        "copies.rj",
        "type B struct { v: [i32; 4] }\nfn main(): i32 {\n  a: B\n  b: B\n  a = B(v: [0; 4])\n  b = a\n  return a.v[0] + b.v[0]\n}\n",
    );

    let (ok, stdout, _) = rjc(&["--json", "--copies", path.to_str().unwrap()]);

    assert!(ok, "notes don't fail compilation");
    assert!(stdout.contains(r#""code":"N0001","title":"copy kept","severity":"note""#), "{stdout}");
    assert!(stdout.contains("copies `a` into `b`"), "{stdout}");
    assert!(stdout.contains(r#""line":6"#), "{stdout}");
}

#[test]
fn fmt_prints_and_writes_the_canonical_form() {
    let path = source_file("fmt.rj", "fn main():i32{return 1+1}\n");
    let canonical = "fn main(): i32 {\n  return 1 + 1\n}\n";

    let (ok, stdout, _) = rjc(&["--fmt", path.to_str().unwrap()]);
    assert!(ok);
    assert_eq!(stdout, canonical);

    let (ok, _, _) = rjc(&["--fmt", "--write", path.to_str().unwrap()]);
    assert!(ok);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), canonical);
}
