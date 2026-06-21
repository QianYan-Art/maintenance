use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn maintenance() -> Command {
    Command::new(env!("CARGO_BIN_EXE_maintenance"))
}

fn temp_project(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_millis();
    let path = std::env::temp_dir().join(format!(
        "maintenance-{name}-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create temp project");
    path
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, text).expect("write fixture");
}

fn latest_run(project: &Path) -> PathBuf {
    let runs = project.join(".doc-maintenance").join("runs");
    let mut entries = fs::read_dir(&runs)
        .expect("read runs")
        .map(|entry| entry.expect("entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    entries.pop().expect("latest run")
}

#[test]
fn route_discovers_dev_docs_without_record_docs() {
    let project = temp_project("route-dev-defaults");
    write(&project.join("README.md"), "PROJECT_README_SECRET_BODY");
    write(&project.join("docs").join("guide.md"), "GUIDE_SECRET_BODY");
    write(
        &project.join("kbase").join("loop-note.md"),
        "KBASE_SECRET_BODY",
    );

    let output = maintenance()
        .args(["route", "--project"])
        .arg(&project)
        .arg("--plain")
        .output()
        .expect("run route");

    assert!(output.status.success());
    let run = latest_run(&project);
    let packet = fs::read_to_string(run.join("packet.md")).expect("packet");
    let manifest = fs::read_to_string(run.join("manifest.json")).expect("manifest");

    assert!(packet.contains("README.md"));
    assert!(packet.contains("docs/guide.md"));
    assert!(!packet.contains("PROJECT_README_SECRET_BODY"));
    assert!(!packet.contains("GUIDE_SECRET_BODY"));
    assert!(!packet.contains("kbase"));
    assert!(manifest.contains("\"schema_version\": 1"));
}

#[test]
fn route_uses_explicit_record_docs_and_archived_lane() {
    let project = temp_project("route-record-docs");
    write(&project.join("README.md"), "README body");
    write(&project.join("docs").join("guide.md"), "Guide body");
    write(
        &project.join("records").join("loop-note.md"),
        "KBase body should not inline",
    );
    write(
        &project.join("records").join("other.md"),
        "Other body should not inline",
    );
    write(
        &project.join("records").join("archived").join("loop-old.md"),
        "Archived body should not inline",
    );

    let output = maintenance()
        .args(["route", "--project"])
        .arg(&project)
        .args([
            "--record-docs",
            "records",
            "--summary-source",
            "README.md",
            "--topic",
            "loop",
            "--plain",
        ])
        .output()
        .expect("run route");

    assert!(output.status.success());
    let run = latest_run(&project);
    let packet = fs::read_to_string(run.join("packet.md")).expect("packet");
    let prompt = fs::read_to_string(run.join("subagent-prompt.md")).expect("prompt");

    assert!(packet.contains("Current Dev Docs"));
    assert!(packet.contains("KBase Records"));
    assert!(packet.contains("Archived Records"));
    assert!(packet.contains("records/loop-note.md"));
    assert!(!packet.contains("records/other.md"));
    assert!(packet.contains("records/archived"));
    assert!(!packet.contains("KBase body should not inline"));
    assert!(prompt.contains("Do not edit files"));
    assert!(prompt.contains("path:line"));
}
