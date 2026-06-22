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
fn init_writes_config_without_overwriting_and_empty_fields_keep_defaults() {
    let project = temp_project("init-config");
    write(&project.join("README.md"), "Readme body");
    write(&project.join("docs").join("guide.md"), "Guide body");

    let first = maintenance()
        .args(["init", "--project"])
        .arg(&project)
        .arg("--plain")
        .output()
        .expect("run init");
    assert!(first.status.success());

    let config_path = project.join(".doc-maintenance").join("config.toml");
    let config = fs::read_to_string(&config_path).expect("config");
    assert!(config.contains("# 开发文档"));
    assert!(config.contains("dev_docs = []"));
    assert!(config.contains("record_docs = []"));
    assert!(config.contains("topic = []"));

    let second = maintenance()
        .args(["init", "--project"])
        .arg(&project)
        .arg("--plain")
        .output()
        .expect("run init again");
    assert!(second.status.success());
    assert!(String::from_utf8_lossy(&second.stdout).contains("config already exists"));
    assert_eq!(
        fs::read_to_string(&config_path).expect("config again"),
        config
    );

    let route = maintenance()
        .args(["route", "--project"])
        .arg(&project)
        .arg("--plain")
        .output()
        .expect("run route");
    assert!(route.status.success());
    let packet = fs::read_to_string(latest_run(&project).join("packet.md")).expect("packet");
    assert!(packet.contains("README.md"));
    assert!(packet.contains("docs/guide.md"));
}

#[test]
fn route_reads_config_defaults_and_cli_overrides_them() {
    let project = temp_project("route-config");
    write(&project.join("README.md"), "Default readme");
    write(
        &project.join("manuals").join("configured.md"),
        "Configured doc",
    );
    write(&project.join("manuals").join("cli.md"), "Cli doc");
    write(&project.join("records").join("loop.md"), "Loop record");
    write(
        &project.join("records-cli").join("manual.md"),
        "Manual record",
    );
    write(
        &project.join(".doc-maintenance").join("config.toml"),
        r#"dev_docs = ["manuals/configured.md"]
record_docs = ["records"]
topic = ["loop"]
"#,
    );

    let configured = maintenance()
        .args(["route", "--project"])
        .arg(&project)
        .arg("--plain")
        .output()
        .expect("run configured route");
    assert!(configured.status.success());
    let packet = fs::read_to_string(latest_run(&project).join("packet.md")).expect("packet");
    assert!(packet.contains("manuals/configured.md"));
    assert!(packet.contains("records/loop.md"));
    assert!(!packet.contains("README.md"));
    assert!(!packet.contains("records-cli/manual.md"));

    let overridden = maintenance()
        .args(["route", "--project"])
        .arg(&project)
        .args([
            "--dev-docs",
            "manuals/cli.md",
            "--record-docs",
            "records-cli",
            "--topic",
            "manual",
            "--plain",
        ])
        .output()
        .expect("run overridden route");
    assert!(overridden.status.success());
    let packet = fs::read_to_string(latest_run(&project).join("packet.md")).expect("packet");
    assert!(packet.contains("manuals/cli.md"));
    assert!(packet.contains("records-cli/manual.md"));
    assert!(!packet.contains("manuals/configured.md"));
    assert!(!packet.contains("records/loop.md"));
}

#[test]
fn closeout_reads_configured_dev_docs_by_default() {
    let project = temp_project("closeout-config");
    write(
        &project.join("manuals").join("configured.md"),
        "Document OLD_ENV here.\n",
    );
    write(
        &project.join(".doc-maintenance").join("config.toml"),
        r#"dev_docs = ["manuals/configured.md"]
record_docs = []
topic = []
"#,
    );
    write(
        &project.join("change.json"),
        r#"{
  "files": [
    {
      "path": "src/app.rs",
      "removed": ["let old = \"OLD_ENV\";"],
      "added": ["let new = \"NEW_ENV\";"]
    }
  ]
}
"#,
    );

    let output = maintenance()
        .args(["closeout", "--project"])
        .arg(&project)
        .args(["--change-manifest", "change.json", "--plain"])
        .output()
        .expect("run closeout");
    assert!(output.status.success());
    let packet = fs::read_to_string(latest_run(&project).join("packet.md")).expect("packet");
    assert!(packet.contains("manuals/configured.md:1"));
}

#[test]
fn gitignore_keeps_doc_maintenance_config_local() {
    let gitignore = fs::read_to_string(".gitignore").expect("read gitignore");

    assert!(gitignore
        .lines()
        .any(|line| line.trim() == ".doc-maintenance/"));
}
