use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn maintenance() -> Command {
    Command::new(env!("CARGO_BIN_EXE_maintenance"))
}

fn temp_project(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
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

fn manifest_json(project: &Path) -> serde_json::Value {
    let manifest = fs::read_to_string(latest_run(project).join("manifest.json")).expect("manifest");
    serde_json::from_str(&manifest).expect("manifest json")
}

fn change_manifest(project: &Path, path: &str, removed: &[&str], added: &[&str]) {
    let file = serde_json::json!({
        "files": [{ "path": path, "removed": removed, "added": added }]
    });
    write(
        &project.join("change.json"),
        &serde_json::to_string_pretty(&file).expect("json"),
    );
}

fn run(project: &Path, args: &[&str]) -> Output {
    let mut command = maintenance();
    command.arg(args[0]).arg("--project").arg(project);
    command.args(&args[1..]).arg("--plain");
    command.output().expect("run maintenance")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn string_list(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| item.as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}

// Give each closeout run its own millisecond run id.
fn next_millisecond() {
    std::thread::sleep(std::time::Duration::from_millis(5));
}

#[test]
fn verify_reports_record_doc_stale_tokens_as_advisory() {
    let project = temp_project("advisory-project");
    let vault = temp_project("advisory-vault");
    write(&project.join("README.md"), "Nothing about the old env.\n");
    write(
        &vault.join("2026-09-25_迁移记录.md"),
        "History: APP_OLD_TOKEN was removed in this release.\n",
    );
    change_manifest(
        &project,
        "src/app.rs",
        &["let old = std::env::var(\"APP_OLD_TOKEN\").unwrap();"],
        &[],
    );

    let closeout = run(
        &project,
        &[
            "closeout",
            "--change-manifest",
            "change.json",
            "--record-docs",
            vault.to_str().expect("utf8 path"),
        ],
    );
    assert!(closeout.status.success());
    let packet = fs::read_to_string(latest_run(&project).join("packet.md")).expect("packet");
    assert!(packet.contains("`stale (advisory)` `APP_OLD_TOKEN`"));

    let verify = run(&project, &["verify"]);
    assert!(verify.status.success(), "stdout:\n{}", stdout(&verify));
    assert!(stdout(&verify).contains("stale_advisory: APP_OLD_TOKEN"));
    let outcome =
        fs::read_to_string(latest_run(&project).join("outcome.json")).expect("outcome.json");
    assert!(outcome.contains("\"result\": \"passed\""));
    assert!(outcome.contains("stale_advisory"));

    let report = run(&project, &["report", "--last", "1"]);
    assert!(stdout(&report).contains("advisory=1"));
}

#[test]
fn run_scoped_waiver_applies_only_to_its_closeout_run() {
    let project = temp_project("run-waiver");
    write(&project.join("README.md"), "No token documented.\n");
    change_manifest(
        &project,
        "src/app.rs",
        &[],
        &["let token = std::env::var(\"APP_RUN_TOKEN\").unwrap();"],
    );

    assert!(
        run(&project, &["closeout", "--change-manifest", "change.json"])
            .status
            .success()
    );
    let first_run = latest_run(&project)
        .file_name()
        .expect("run id")
        .to_string_lossy()
        .to_string();
    assert_eq!(run(&project, &["verify"]).status.code(), Some(2));

    let waive = run(
        &project,
        &["waive", "APP_RUN_TOKEN", "--reason", "internal only"],
    );
    assert!(waive.status.success());
    assert!(stdout(&waive).contains(&format!("run {first_run}")));
    let waivers =
        fs::read_to_string(project.join(".doc-maintenance").join("waivers.toml")).expect("waivers");
    assert!(waivers.contains("scope = \"run\""));
    assert!(waivers.contains(&format!("run = \"{first_run}\"")));
    assert!(run(&project, &["verify"]).status.success());

    next_millisecond();
    assert!(
        run(&project, &["closeout", "--change-manifest", "change.json"])
            .status
            .success()
    );
    let manifest = manifest_json(&project);
    assert!(
        string_list(&manifest["closeout"]["missing_tokens"]).contains(&"APP_RUN_TOKEN".to_string())
    );
    assert_eq!(run(&project, &["verify"]).status.code(), Some(2));
}

#[test]
fn run_scoped_waiver_requires_a_closeout_run() {
    let project = temp_project("run-waiver-without-closeout");
    let waive = run(&project, &["waive", "APP_TOKEN", "--reason", "noise"]);
    assert_eq!(waive.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&waive.stderr).contains("--scope project"));
}

#[test]
fn expired_project_waiver_no_longer_applies() {
    let project = temp_project("expired-waiver");
    write(&project.join("README.md"), "No token documented.\n");
    let waive = run(
        &project,
        &[
            "waive",
            "APP_EXPIRED",
            "--reason",
            "temporary",
            "--scope",
            "project",
            "--expires",
            "2000-01-01",
        ],
    );
    assert!(waive.status.success());
    let bad_date = run(
        &project,
        &[
            "waive",
            "APP_OTHER",
            "--reason",
            "temporary",
            "--scope",
            "project",
            "--expires",
            "2000-13-40",
        ],
    );
    assert_eq!(bad_date.status.code(), Some(1));

    change_manifest(
        &project,
        "src/app.rs",
        &[],
        &["let token = std::env::var(\"APP_EXPIRED\").unwrap();"],
    );
    assert!(
        run(&project, &["closeout", "--change-manifest", "change.json"])
            .status
            .success()
    );
    let manifest = manifest_json(&project);
    assert!(
        string_list(&manifest["closeout"]["missing_tokens"]).contains(&"APP_EXPIRED".to_string())
    );
    assert!(manifest["closeout"]["ignored_tokens"]
        .as_array()
        .expect("ignored tokens")
        .is_empty());
}

#[test]
fn tokens_from_test_paths_are_low_confidence() {
    let project = temp_project("test-path-tokens");
    write(&project.join("README.md"), "Usage.\n");
    let file = serde_json::json!({
        "files": [
            { "path": "tests/cli_tests.rs", "removed": [], "added": ["args([\"--only-in-tests\"])"] },
            { "path": "web/src/app.test.ts", "removed": [], "added": ["process.env.APP_TEST_ONLY"] },
            { "path": "src/main.rs", "removed": [], "added": ["#[arg(long = \"--real-flag\")]"] }
        ]
    });
    write(
        &project.join("change.json"),
        &serde_json::to_string_pretty(&file).expect("json"),
    );

    assert!(
        run(&project, &["closeout", "--change-manifest", "change.json"])
            .status
            .success()
    );
    let closeout = &manifest_json(&project)["closeout"];
    let new_tokens = string_list(&closeout["new_tokens"]);
    assert!(new_tokens.contains(&"--real-flag".to_string()));
    assert!(!new_tokens.contains(&"--only-in-tests".to_string()));
    assert!(!new_tokens.contains(&"APP_TEST_ONLY".to_string()));
    let low = closeout["low_confidence_tokens"]
        .as_array()
        .expect("low confidence");
    assert!(low
        .iter()
        .any(|token| token["token"] == "--only-in-tests" && token["evidence"] == "path:test"));
}

#[test]
fn packet_groups_repeated_hits_and_splits_documented_tokens() {
    let project = temp_project("grouped-hits");
    write(
        &project.join("README.md"),
        "Run with --verbose.\nThe --verbose flag prints more.\nAlso --verbose here.\n",
    );
    change_manifest(
        &project,
        "src/main.rs",
        &[],
        &[
            "#[arg(long = \"--verbose\")]",
            "#[arg(long = \"--brand-new\")]",
        ],
    );

    assert!(
        run(&project, &["closeout", "--change-manifest", "change.json"])
            .status
            .success()
    );
    let packet = fs::read_to_string(latest_run(&project).join("packet.md")).expect("packet");
    assert!(packet.contains("`update` `--verbose` at `README.md` lines 1, 2, 3"));
    assert_eq!(packet.matches("`update` `--verbose`").count(), 1);
    assert!(packet.contains("  - already documented: `--verbose`"));
    assert!(packet.contains("  - not yet documented: `--brand-new`"));
}

#[test]
fn compare_builds_a_change_set_from_a_file_pair() {
    let project = temp_project("compare");
    write(
        &project.join("README.md"),
        "Set server.old_port before starting.\n",
    );
    write(
        &project.join("backup").join("app.toml.bak"),
        "name = \"demo\"\nserver.old_port = 8080\n",
    );
    write(
        &project.join("app.toml"),
        "name = \"demo\"\nserver.new_port = 8080\n",
    );

    let closeout = run(
        &project,
        &["closeout", "--compare", "backup/app.toml.bak", "app.toml"],
    );
    assert!(closeout.status.success(), "stdout:\n{}", stdout(&closeout));
    let closeout = &manifest_json(&project)["closeout"];
    assert_eq!(closeout["source"]["kind"], "compare");
    assert_eq!(string_list(&closeout["changed_files"]), vec!["app.toml"]);
    assert!(string_list(&closeout["new_tokens"]).contains(&"server.new_port".to_string()));
    assert!(string_list(&closeout["removed_tokens"]).contains(&"server.old_port".to_string()));
    assert!(!string_list(&closeout["new_tokens"]).contains(&"name".to_string()));

    let verify = run(&project, &["verify"]);
    assert_eq!(verify.status.code(), Some(2));
    assert!(stdout(&verify).contains("stale_remaining: server.old_port"));
}

#[test]
fn compare_rejects_a_pair_where_neither_file_exists() {
    let project = temp_project("compare-missing");
    let closeout = run(
        &project,
        &[
            "closeout",
            "--compare",
            "nope-before.toml",
            "nope-after.toml",
        ],
    );
    assert_eq!(closeout.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&closeout.stderr).contains("neither"));
}

#[test]
fn version_flag_prints_the_crate_version() {
    let output = maintenance()
        .arg("--version")
        .output()
        .expect("run --version");
    assert!(output.status.success());
    assert!(stdout(&output).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn report_counts_input_warnings() {
    let project = temp_project("report-warnings");
    write(&project.join("README.md"), "Docs.\n");
    change_manifest(&project, "src/app.rs", &[], &["let x = 1;"]);
    let missing = project.join("records").join("not-yet.md");
    assert!(run(
        &project,
        &[
            "closeout",
            "--change-manifest",
            "change.json",
            "--record-docs",
            missing.to_str().expect("utf8 path"),
        ],
    )
    .status
    .success());
    let report = run(&project, &["report", "--last", "1"]);
    assert!(stdout(&report).contains("warnings=2"));
}
