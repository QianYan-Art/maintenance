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
        "Record docs body should not inline",
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
    assert!(packet.contains("Record Docs"));
    assert!(packet.contains("Archived Records"));
    assert!(packet.contains("records/loop-note.md"));
    assert!(!packet.contains("records/other.md"));
    assert!(packet.contains("records/archived"));
    assert!(!packet.contains("Record docs body should not inline"));
    assert!(prompt.contains("Do not edit files"));
    assert!(prompt.contains("path:line"));
}

fn route_manifest(project: &Path, args: &[&str]) -> (serde_json::Value, String, String) {
    let output = maintenance()
        .args(["route", "--project"])
        .arg(project)
        .args(args)
        .arg("--plain")
        .output()
        .expect("run route");
    assert!(output.status.success());
    let run = latest_run(project);
    let manifest = fs::read_to_string(run.join("manifest.json")).expect("manifest");
    let packet = fs::read_to_string(run.join("packet.md")).expect("packet");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    (
        serde_json::from_str(&manifest).expect("manifest json"),
        packet,
        stdout,
    )
}

fn record_candidates(manifest: &serde_json::Value) -> Vec<String> {
    manifest["candidates"]
        .as_array()
        .expect("candidates")
        .iter()
        .filter(|candidate| candidate["lane"] == "Record Docs")
        .map(|candidate| candidate["path"].as_str().expect("path").to_string())
        .collect()
}

fn warning_kinds(manifest: &serde_json::Value) -> Vec<String> {
    manifest["input_warnings"]
        .as_array()
        .map(|warnings| {
            warnings
                .iter()
                .map(|warning| warning["kind"].as_str().expect("kind").to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn route_keeps_explicit_record_file_regardless_of_topic() {
    let project = temp_project("route-explicit-cjk");
    let vault = temp_project("route-explicit-cjk-vault");
    let record = vault
        .join("a-本机配置")
        .join("2026-09-11_Codex(含codex++)、Pi本机共同配置记录.md");
    write(&record, "记录正文");

    let (manifest, packet, _) = route_manifest(
        &project,
        &[
            "--record-docs",
            record.to_str().expect("utf8 path"),
            "--topic",
            "codex config approval_mode",
        ],
    );

    assert_eq!(record_candidates(&manifest).len(), 1);
    assert!(packet.contains("本机共同配置记录.md"));
    assert!(warning_kinds(&manifest).is_empty());
    assert!(!packet.contains("Input Warnings"));
}

#[test]
fn route_topic_matches_any_term_case_insensitively() {
    let project = temp_project("route-topic-terms");
    let vault = temp_project("route-topic-terms-vault");
    write(
        &vault.join("2026-09-24_SafetyRAISE迁移与知识库开源边界记录.md"),
        "body",
    );
    write(&vault.join("2026-09-11_Komari升级记录.md"), "body");

    let (manifest, _, _) = route_manifest(
        &project,
        &[
            "--record-docs",
            vault.to_str().expect("utf8 path"),
            "--topic",
            "safetyraise release governance",
        ],
    );

    let records = record_candidates(&manifest);
    assert_eq!(records.len(), 1);
    assert!(records[0].contains("SafetyRAISE"));

    let (manifest, _, _) = route_manifest(
        &project,
        &[
            "--record-docs",
            vault.to_str().expect("utf8 path"),
            "--topic",
            "迁移，升级",
        ],
    );
    assert_eq!(record_candidates(&manifest).len(), 2);
}

#[test]
fn route_warns_when_explicit_record_path_is_missing() {
    let project = temp_project("route-missing-record");
    let vault = temp_project("route-missing-record-vault");
    let missing = vault.join("2026-09-16_尚未创建的发布记录.md");

    let (manifest, packet, stdout) = route_manifest(
        &project,
        &["--record-docs", missing.to_str().expect("utf8 path")],
    );

    assert!(record_candidates(&manifest).is_empty());
    let kinds = warning_kinds(&manifest);
    assert!(kinds.contains(&"missing_path".to_string()));
    assert!(kinds.contains(&"no_record_docs".to_string()));
    assert!(packet.contains("## Input Warnings"));
    assert!(packet.contains("尚未创建的发布记录.md"));
    assert!(packet.contains("create the document first"));
    assert!(stdout.contains("missing_path"));
}

#[test]
fn route_warns_when_topic_filters_out_every_record_doc() {
    let project = temp_project("route-topic-empty");
    let vault = temp_project("route-topic-empty-vault");
    write(&vault.join("2026-09-11_Komari升级记录.md"), "body");
    write(&vault.join("2026-09-12_目录规范化记录.md"), "body");

    let (manifest, packet, _) = route_manifest(
        &project,
        &[
            "--record-docs",
            vault.to_str().expect("utf8 path"),
            "--topic",
            "hermes",
        ],
    );

    assert!(record_candidates(&manifest).is_empty());
    assert_eq!(warning_kinds(&manifest), vec!["no_record_docs".to_string()]);
    assert!(packet.contains("2 document(s) were filtered out by topic hermes"));
}

#[test]
fn route_omits_input_warnings_when_inputs_resolve() {
    let project = temp_project("route-no-warnings");
    write(&project.join("README.md"), "README body");

    let (manifest, packet, _) = route_manifest(&project, &[]);

    assert!(manifest.get("input_warnings").is_none());
    assert!(!packet.contains("Input Warnings"));
}
