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

fn manifest_json(project: &Path) -> serde_json::Value {
    let manifest = fs::read_to_string(latest_run(project).join("manifest.json")).expect("manifest");
    serde_json::from_str(&manifest).expect("manifest json")
}

fn git(project: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(project)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

#[test]
fn closeout_change_manifest_and_verify_close_the_loop() {
    let project = temp_project("closeout-manifest");
    write(
        &project.join("README.md"),
        "Configure OLD_ENV before launching.\n",
    );
    write(
        &project.join("change.json"),
        r#"{
  "files": [
    {
      "path": "src/app.rs",
      "removed": ["let old = \"OLD_ENV\";"],
      "added": ["let new = \"NEW_ENV\"; let flag = \"--new-flag\"; let key = \"service.new_url\";"]
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
    let run = latest_run(&project);
    let packet = fs::read_to_string(run.join("packet.md")).expect("packet");
    let manifest = fs::read_to_string(run.join("manifest.json")).expect("manifest");

    assert!(packet.contains("src/app.rs"));
    assert!(packet.contains("NEW_ENV"));
    assert!(packet.contains("OLD_ENV"));
    assert!(packet.contains("stale"));
    assert!(packet.contains("README.md:1"));
    assert!(manifest.contains("\"command\": \"closeout\""));
    assert!(manifest.contains("\"missing_tokens\""));

    let failed_verify = maintenance()
        .args(["verify", "--project"])
        .arg(&project)
        .arg("--plain")
        .output()
        .expect("run verify");
    assert_eq!(failed_verify.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&failed_verify.stdout);
    assert!(stdout.contains("stale_remaining: OLD_ENV"));
    assert!(stdout.contains("missing_remaining: NEW_ENV"));

    write(
        &project.join("README.md"),
        "Configure NEW_ENV, --new-flag, and service.new_url before launching.\n",
    );
    let passed_verify = maintenance()
        .args(["verify", "--project"])
        .arg(&project)
        .arg("--plain")
        .output()
        .expect("run verify again");
    assert!(passed_verify.status.success());
}

#[test]
fn closeout_supports_git_uncommitted_and_since_sources() {
    let project = temp_project("closeout-git");
    write(&project.join("README.md"), "Document OLD_ENV.\n");
    write(&project.join("src").join("app.txt"), "OLD_ENV\n");
    git(&project, &["init"]);
    git(&project, &["config", "user.email", "test@example.invalid"]);
    git(&project, &["config", "user.name", "Test User"]);
    git(&project, &["add", "."]);
    git(&project, &["commit", "-m", "initial"]);

    write(&project.join("src").join("app.txt"), "NEW_ENV\n");
    write(&project.join("src").join("new-file.txt"), "UNTRACKED_ENV\n");
    let uncommitted = maintenance()
        .args(["closeout", "--project"])
        .arg(&project)
        .args(["--git", "uncommitted", "--plain"])
        .output()
        .expect("run closeout git uncommitted");
    assert!(uncommitted.status.success());
    let packet = fs::read_to_string(latest_run(&project).join("packet.md")).expect("packet");
    assert!(packet.contains("git_uncommitted"));
    assert!(packet.contains("NEW_ENV"));
    assert!(packet.contains("OLD_ENV"));
    assert!(packet.contains("UNTRACKED_ENV"));

    git(&project, &["add", "."]);
    git(&project, &["commit", "-m", "change env"]);
    let since = maintenance()
        .args(["closeout", "--project"])
        .arg(&project)
        .args(["--since", "HEAD~1", "--plain"])
        .output()
        .expect("run closeout since");
    assert!(since.status.success());
    let packet = fs::read_to_string(latest_run(&project).join("packet.md")).expect("packet");
    assert!(packet.contains("git_since"));
    assert!(packet.contains("NEW_ENV"));
    assert!(packet.contains("OLD_ENV"));
}

#[test]
fn closeout_rejects_missing_or_path_only_change_sources() {
    let project = temp_project("closeout-source-errors");
    write(&project.join("README.md"), "No git here.\n");

    let non_git = maintenance()
        .args(["closeout", "--project"])
        .arg(&project)
        .args(["--git", "uncommitted", "--plain"])
        .output()
        .expect("run non-git closeout");
    assert_eq!(non_git.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&non_git.stdout).contains("needs_input: changed_source"));

    let path_only = maintenance()
        .args(["closeout", "--project"])
        .arg(&project)
        .args(["--changed-files", "src/app.rs", "--plain"])
        .output()
        .expect("run path-only closeout");
    assert!(!path_only.status.success());
    assert!(String::from_utf8_lossy(&path_only.stderr).contains("unexpected argument"));
}

#[test]
fn closeout_pack_is_bounded_and_contextual() {
    let project = temp_project("closeout-pack");
    let mut readme = String::from("# Config\n\nUse OLD_ENV for startup.\n");
    for index in 0..80 {
        readme.push_str(&format!("FILLER_LINE_{index}\n"));
    }
    write(&project.join("README.md"), &readme);
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
        .args([
            "--change-manifest",
            "change.json",
            "--pack",
            "--max-lines",
            "30",
            "--plain",
        ])
        .output()
        .expect("run closeout pack");

    assert!(output.status.success());
    let pack = fs::read_to_string(latest_run(&project).join("pack.md")).expect("pack");
    assert!(pack.lines().count() <= 30);
    assert!(pack.contains("OLD_ENV"));
    assert!(pack.contains("title: # Config"));
    assert!(!pack.contains("FILLER_LINE_50"));
}

#[test]
fn closeout_excludes_tokens_that_are_both_added_and_removed() {
    let project = temp_project("closeout-token-diff");
    write(&project.join("README.md"), "No option documented yet.\n");
    write(
        &project.join("change.json"),
        r#"{
  "files": [
    {
      "path": "README.md",
      "removed": ["cargo run -- closeout --pack --max-lines 100"],
      "added": ["cargo run -- closeout --pack --max-lines 200"]
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
    let manifest = manifest_json(&project);
    let closeout = &manifest["closeout"];
    assert!(!closeout["removed_tokens"]
        .as_array()
        .expect("removed tokens")
        .iter()
        .any(|token| token == "--max-lines"));
    assert!(!closeout["missing_tokens"]
        .as_array()
        .expect("missing tokens")
        .iter()
        .any(|token| token == "--max-lines"));
    let packet = fs::read_to_string(latest_run(&project).join("packet.md")).expect("packet");
    assert!(!packet.contains("stale` `--max-lines"));
}

#[test]
fn closeout_extracts_config_keys_only_from_config_files() {
    let project = temp_project("closeout-config-file-types");
    write(&project.join("README.md"), "No config documented yet.\n");
    write(
        &project.join("change.json"),
        r#"{
  "files": [
    {
      "path": "config/app.toml",
      "removed": [],
      "added": [
        "server.workers = 4",
        "log.level = \"debug\"",
        "env = \"CONFIG_ENV\"",
        "flag = \"--config-flag\""
      ]
    },
    {
      "path": "src/lib.rs",
      "removed": [],
      "added": [
        "self.method();",
        "std.fs();",
        "let timeout = service.timeout;",
        "let env = \"CODE_ENV\";",
        "let flag = \"--code-flag\";"
      ]
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
    let manifest = manifest_json(&project);
    let closeout = &manifest["closeout"];
    let new_tokens = closeout["new_tokens"].as_array().expect("new tokens");
    for token in [
        "server.workers",
        "log.level",
        "CONFIG_ENV",
        "CODE_ENV",
        "--config-flag",
        "--code-flag",
    ] {
        assert!(new_tokens.iter().any(|value| value == token), "{token}");
    }
    for token in ["self.method", "std.fs", "service.timeout"] {
        assert!(!new_tokens.iter().any(|value| value == token), "{token}");
    }
}

#[test]
fn verify_checks_stale_tokens_against_impact_paths_only() {
    let project = temp_project("verify-impact-paths");
    write(&project.join("README.md"), "Document OLD_ENV here.\n");
    write(
        &project.join("docs").join("other.md"),
        "No token here yet.\n",
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

    write(&project.join("README.md"), "Document NEW_ENV here.\n");
    write(
        &project.join("docs").join("other.md"),
        "This separate doc may mention OLD_ENV without being the stale impact path.\n",
    );

    let verify = maintenance()
        .args(["verify", "--project"])
        .arg(&project)
        .arg("--plain")
        .output()
        .expect("run verify");
    assert!(
        verify.status.success(),
        "verify stdout:\n{}\nverify stderr:\n{}",
        String::from_utf8_lossy(&verify.stdout),
        String::from_utf8_lossy(&verify.stderr)
    );
}

#[test]
fn verify_checks_missing_tokens_against_recorded_target_path() {
    let project = temp_project("verify-missing-target-path");
    write(&project.join("README.md"), "No new token here.\n");
    write(
        &project.join("docs").join("other.md"),
        "No token here either.\n",
    );
    write(
        &project.join("change.json"),
        r#"{
  "files": [
    {
      "path": "src/app.rs",
      "removed": [],
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
    let manifest = manifest_json(&project);
    let targets = manifest["closeout"]["missing_targets"]
        .as_array()
        .expect("missing targets");
    assert!(targets
        .iter()
        .any(|target| target["token"] == "NEW_ENV" && target["path"] == "README.md"));

    write(&project.join("README.md"), "Still no new token here.\n");
    write(
        &project.join("docs").join("other.md"),
        "Document NEW_ENV here.\n",
    );
    let wrong_path_verify = maintenance()
        .args(["verify", "--project"])
        .arg(&project)
        .arg("--plain")
        .output()
        .expect("run verify wrong path");
    assert_eq!(wrong_path_verify.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&wrong_path_verify.stdout).contains("missing_remaining: NEW_ENV")
    );

    write(&project.join("README.md"), "Document NEW_ENV here.\n");
    let right_path_verify = maintenance()
        .args(["verify", "--project"])
        .arg(&project)
        .arg("--plain")
        .output()
        .expect("run verify right path");
    assert!(
        right_path_verify.status.success(),
        "verify stdout:\n{}\nverify stderr:\n{}",
        String::from_utf8_lossy(&right_path_verify.stdout),
        String::from_utf8_lossy(&right_path_verify.stderr)
    );
}

#[test]
fn verify_keeps_missing_fallback_for_old_manifests() {
    let project = temp_project("verify-old-missing-manifest");
    write(&project.join("README.md"), "Document NEW_ENV here.\n");
    let manifest = serde_json::json!({
        "schema_version": 1,
        "command": "closeout",
        "project": project.display().to_string().replace('\\', "/"),
        "inputs": {
            "dev_docs": ["README.md"],
            "record_docs": [],
            "summary_source": [],
            "topic": []
        },
        "candidates": [
            {
                "path": "README.md",
                "lane": "Current Dev Docs",
                "reason": "explicit document path",
                "archived": false
            }
        ],
        "rules": [],
        "closeout": {
            "source": {
                "kind": "change_manifest",
                "detail": "legacy"
            },
            "changed_files": ["src/app.rs"],
            "changed_categories": ["env"],
            "new_tokens": ["NEW_ENV"],
            "removed_tokens": [],
            "missing_tokens": ["NEW_ENV"],
            "possible_doc_impact": []
        }
    });
    write(
        &project
            .join(".doc-maintenance")
            .join("runs")
            .join("1")
            .join("manifest.json"),
        &serde_json::to_string_pretty(&manifest).expect("manifest"),
    );

    let verify = maintenance()
        .args(["verify", "--project"])
        .arg(&project)
        .arg("--plain")
        .output()
        .expect("run verify");
    assert!(
        verify.status.success(),
        "verify stdout:\n{}\nverify stderr:\n{}",
        String::from_utf8_lossy(&verify.stdout),
        String::from_utf8_lossy(&verify.stderr)
    );
}

#[test]
fn verify_selects_latest_manifest_by_closeout_payload() {
    let project = temp_project("verify-closeout-manifest");
    write(&project.join("README.md"), "Document OLD_ENV here.\n");
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
    write(&project.join("README.md"), "Document NEW_ENV here.\n");

    let newer_run = project
        .join(".doc-maintenance")
        .join("runs")
        .join("9999999999999");
    let fake_manifest = serde_json::json!({
        "schema_version": 1,
        "command": "closeout",
        "project": project.display().to_string().replace('\\', "/"),
        "inputs": {
            "dev_docs": [],
            "record_docs": [],
            "summary_source": [],
            "topic": []
        },
        "candidates": [],
        "rules": []
    });
    write(
        &newer_run.join("manifest.json"),
        &serde_json::to_string_pretty(&fake_manifest).expect("fake manifest"),
    );

    let verify = maintenance()
        .args(["verify", "--project"])
        .arg(&project)
        .arg("--plain")
        .output()
        .expect("run verify");
    assert!(
        verify.status.success(),
        "verify stdout:\n{}\nverify stderr:\n{}",
        String::from_utf8_lossy(&verify.stdout),
        String::from_utf8_lossy(&verify.stderr)
    );
}
