use std::process::Command;

fn maintenance() -> Command {
    Command::new(env!("CARGO_BIN_EXE_maintenance"))
}

#[test]
fn help_lists_core_commands() {
    let output = maintenance()
        .arg("--help")
        .output()
        .expect("run maintenance --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("route"));
    assert!(stdout.contains("closeout"));
    assert!(stdout.contains("verify"));
}

#[test]
fn plain_closeout_has_no_ansi_and_requires_change_source() {
    let output = maintenance()
        .args(["closeout", "--project", ".", "--plain"])
        .output()
        .expect("run maintenance closeout");

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("needs_input: changed_source"));
    assert!(!stdout.contains('\u{1b}'));
    assert!(!stdout.contains("Yan Maintenance"));
}

#[test]
fn route_plain_runs_without_banner() {
    let output = maintenance()
        .args(["route", "--project", ".", "--plain"])
        .output()
        .expect("run maintenance route");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("packet:"));
    assert!(!stdout.contains('\u{1b}'));
    assert!(!stdout.contains("Yan Maintenance"));
}
