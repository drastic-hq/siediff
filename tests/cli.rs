use std::{path::Path, process::Command};
fn fixture(n: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(n)
        .to_str()
        .unwrap()
        .into()
}
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_siediff"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn help_succeeds() {
    assert!(run(&["--help"]).status.success());
}
#[test]
fn same_export_exit_zero_for_postings_only() {
    let p = fixture("base.se");
    let r = run(&["diff", &p, &p]);
    assert_eq!(r.status.code(), Some(0));
    let s = String::from_utf8(r.stdout).unwrap();
    assert!(!s.contains("Fiktiv AB"));
    assert!(s.contains("migration_readiness"));
}
#[test]
fn migration_claim_remains_unknown() {
    let p = fixture("base.se");
    assert_eq!(
        run(&["diff", &p, &p, "--claim", "migration_readiness"])
            .status
            .code(),
        Some(3)
    );
}
#[test]
fn difference_exit_one() {
    assert_eq!(
        run(&["diff", &fixture("base.se"), &fixture("wrong-account.se")])
            .status
            .code(),
        Some(1)
    );
}
#[test]
fn malformed_exit_two() {
    assert_eq!(
        run(&["validate", &fixture("half-cent.se")]).status.code(),
        Some(2)
    );
}
#[test]
fn empty_postings_exit_three() {
    let p = fixture("empty-postings.se");
    assert_eq!(run(&["diff", &p, &p]).status.code(), Some(3));
}
#[test]
fn irrelevant_option_refused() {
    assert_eq!(
        run(&["inspect", &fixture("base.se"), "--mode", "renumbered"])
            .status
            .code(),
        Some(2)
    );
}
#[test]
fn remote_path_refused() {
    let r = run(&["inspect", "https://example.invalid/private.se"]);
    assert_eq!(r.status.code(), Some(2));
    assert!(!String::from_utf8(r.stderr)
        .unwrap()
        .contains("example.invalid"));
}
#[test]
fn refuses_existing_output() {
    let p = fixture("base.se");
    let before = std::fs::read(&p).unwrap();
    let r = run(&["diff", &p, &p, "--output", &fixture("whitespace.se")]);
    assert_eq!(r.status.code(), Some(5));
    assert_eq!(std::fs::read(&p).unwrap(), before);
}
