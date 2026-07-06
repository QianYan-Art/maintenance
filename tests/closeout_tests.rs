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
      "removed": ["let old = std::env::var(\"OLD_ENV\").unwrap();"],
      "added": ["let new = std::env::var(\"NEW_ENV\").unwrap(); let flag = \"--new-flag\"; let key = \"service.new_url\";"]
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
    let prompt = fs::read_to_string(run.join("subagent-prompt.md")).expect("prompt");
    let manifest = fs::read_to_string(run.join("manifest.json")).expect("manifest");

    assert!(packet.contains("src/app.rs"));
    assert!(packet.contains("NEW_ENV"));
    assert!(packet.contains("OLD_ENV"));
    assert!(packet.contains("stale"));
    assert!(packet.contains("README.md:1"));
    assert!(manifest.contains("\"command\": \"closeout\""));
    assert!(manifest.contains("\"missing_tokens\""));
    assert!(prompt.contains("## Change Evidence"));
    assert!(prompt.contains("Changed files"));
    assert!(prompt.contains("NEW_ENV"));
    assert!(prompt.contains("env, high"));
    assert!(prompt.contains("README.md:1"));
    assert!(!prompt.contains("Configure OLD_ENV before launching."));

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
fn closeout_tracks_token_confidence_and_excludes_heredoc_delimiters() {
    let project = temp_project("closeout-token-confidence");
    write(
        &project.join("README.md"),
        "No new runtime settings documented yet.\n",
    );
    write(
        &project.join("change.json"),
        r#"{
  "files": [
    {
      "path": ".env.local",
      "removed": [],
      "added": ["APP_ENV=production"]
    },
    {
      "path": "deploy/docker-compose.yml",
      "removed": [],
      "added": ["environment:", "  APP_PORT: \"8080\""]
    },
    {
      "path": "scripts/setup.sh",
      "removed": [],
      "added": ["cat <<EOF", "echo $SHELL_ENV", "EOF"]
    },
    {
      "path": "src/app.rs",
      "removed": [],
      "added": ["let label = \"NOISE_TOKEN\"; std::env::var(\"APP_SECRET\").unwrap();"]
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
    assert_eq!(manifest["schema_version"].as_u64(), Some(2));
    let closeout = &manifest["closeout"];
    let new_tokens = closeout["new_tokens"].as_array().expect("new tokens");
    for token in ["APP_ENV", "APP_PORT", "APP_SECRET", "SHELL_ENV"] {
        assert!(new_tokens.iter().any(|value| value == token), "{token}");
    }
    for token in ["EOF", "NOISE_TOKEN"] {
        assert!(!new_tokens.iter().any(|value| value == token), "{token}");
    }

    let missing_tokens = closeout["missing_tokens"]
        .as_array()
        .expect("missing tokens");
    assert!(missing_tokens.iter().any(|value| value == "APP_SECRET"));
    assert!(!missing_tokens.iter().any(|value| value == "NOISE_TOKEN"));
    assert!(!missing_tokens.iter().any(|value| value == "EOF"));

    let low_confidence = closeout["low_confidence_tokens"]
        .as_array()
        .expect("low confidence tokens");
    assert!(low_confidence
        .iter()
        .any(|value| value["token"].as_str() == Some("NOISE_TOKEN")
            && value["confidence"].as_str() == Some("low")));
    assert!(!low_confidence
        .iter()
        .any(|value| value["token"].as_str() == Some("EOF")));

    let packet = fs::read_to_string(latest_run(&project).join("packet.md")).expect("packet");
    assert!(packet.contains("Low confidence reference"));
    assert!(packet.contains("NOISE_TOKEN"));
}

#[test]
fn waive_records_reason_and_filters_closeout_and_verify_tokens() {
    let project = temp_project("closeout-waivers");
    write(&project.join("README.md"), "No secret documented here.\n");
    let waive = maintenance()
        .args(["waive", "--project"])
        .arg(&project)
        .args([
            "APP_SECRET",
            "--reason",
            "runtime secret stays out of docs",
            "--plain",
        ])
        .output()
        .expect("run waive");
    assert!(waive.status.success());
    let waive_again = maintenance()
        .args(["waive", "--project"])
        .arg(&project)
        .args([
            "APP_SECRET",
            "--reason",
            "runtime secret stays out of docs",
            "--plain",
        ])
        .output()
        .expect("run waive again");
    assert!(waive_again.status.success());
    assert!(
        String::from_utf8_lossy(&waive_again.stdout).contains("waiver already exists"),
        "stdout:\n{}",
        String::from_utf8_lossy(&waive_again.stdout)
    );
    let waivers =
        fs::read_to_string(project.join(".doc-maintenance").join("waivers.toml")).expect("waivers");
    assert_eq!(waivers.matches("token = \"APP_SECRET\"").count(), 1);
    assert!(waivers.contains("reason = \"runtime secret stays out of docs\""));

    write(
        &project.join("change.json"),
        r#"{
  "files": [
    {
      "path": "src/app.rs",
      "removed": [],
      "added": ["let secret = std::env::var(\"APP_SECRET\").unwrap();"]
    }
  ]
}
"#,
    );
    let closeout = maintenance()
        .args(["closeout", "--project"])
        .arg(&project)
        .args(["--change-manifest", "change.json", "--plain"])
        .output()
        .expect("run closeout");
    assert!(closeout.status.success());
    let manifest = manifest_json(&project);
    let closeout = &manifest["closeout"];
    assert!(!closeout["missing_tokens"]
        .as_array()
        .expect("missing tokens")
        .iter()
        .any(|token| token == "APP_SECRET"));
    assert!(closeout["ignored_tokens"]
        .as_array()
        .expect("ignored tokens")
        .iter()
        .any(|token| token["token"] == "APP_SECRET"
            && token["reason"] == "runtime secret stays out of docs"));

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
fn verify_writes_outcome_and_report_summarizes_runs() {
    let project = temp_project("closeout-report");
    write(&project.join("README.md"), "No report token yet.\n");
    write(
        &project.join("change.json"),
        r#"{
  "files": [
    {
      "path": "src/app.rs",
      "removed": [],
      "added": ["let token = std::env::var(\"APP_REPORT_TOKEN\").unwrap();"]
    }
  ]
}
"#,
    );
    let closeout = maintenance()
        .args(["closeout", "--project"])
        .arg(&project)
        .args(["--change-manifest", "change.json", "--plain"])
        .output()
        .expect("run closeout");
    assert!(closeout.status.success());
    let run = latest_run(&project);

    let unverified_report = maintenance()
        .args(["report", "--project"])
        .arg(&project)
        .args(["--last", "1", "--plain"])
        .output()
        .expect("run report before verify");
    assert!(unverified_report.status.success());
    let stdout = String::from_utf8_lossy(&unverified_report.stdout);
    assert!(stdout.contains("verify=unverified"));
    assert!(stdout.contains("changed=1"));
    assert!(stdout.contains("high=1"));
    assert!(stdout.contains("missing=1"));

    let failed_verify = maintenance()
        .args(["verify", "--project"])
        .arg(&project)
        .arg("--plain")
        .output()
        .expect("run verify");
    assert_eq!(failed_verify.status.code(), Some(2));
    let outcome = fs::read_to_string(run.join("outcome.json")).expect("outcome");
    assert!(outcome.contains("\"result\": \"failed\""));
    assert!(outcome.contains("APP_REPORT_TOKEN"));
    let failed_report = maintenance()
        .args(["report", "--project"])
        .arg(&project)
        .args(["--last", "1", "--plain"])
        .output()
        .expect("run report after failed verify");
    assert!(String::from_utf8_lossy(&failed_report.stdout).contains("verify=failed"));

    write(
        &project.join("README.md"),
        "Document APP_REPORT_TOKEN for local report tests.\n",
    );
    let passed_verify = maintenance()
        .args(["verify", "--project"])
        .arg(&project)
        .arg("--plain")
        .output()
        .expect("run verify again");
    assert!(passed_verify.status.success());
    let passed_report = maintenance()
        .args(["report", "--project"])
        .arg(&project)
        .args(["--last", "1", "--plain"])
        .output()
        .expect("run report after passed verify");
    assert!(String::from_utf8_lossy(&passed_report.stdout).contains("verify=passed"));
}

#[test]
fn closeout_routes_missing_tokens_by_source_affinity() {
    let project = temp_project("closeout-affinity-route");
    write(&project.join("README.md"), "General runtime notes.\n");
    write(
        &project.join("docs").join("deployment.md"),
        "Deployment settings live here.\n",
    );
    write(
        &project.join("change.json"),
        r#"{
  "files": [
    {
      "path": "deployment/docker/config.rs",
      "removed": [],
      "added": ["let limit = std::env::var(\"APP_LOG_MAX_FILE\").unwrap();"]
    }
  ]
}
"#,
    );

    let first = maintenance()
        .args(["closeout", "--project"])
        .arg(&project)
        .args(["--change-manifest", "change.json", "--plain"])
        .output()
        .expect("run closeout");
    assert!(first.status.success());
    let first_manifest = manifest_json(&project);
    let first_targets = first_manifest["closeout"]["missing_targets"]
        .as_array()
        .expect("missing targets");
    assert!(first_targets
        .iter()
        .any(|target| target["token"] == "APP_LOG_MAX_FILE"
            && target["path"] == "docs/deployment.md"));

    let second = maintenance()
        .args(["closeout", "--project"])
        .arg(&project)
        .args(["--change-manifest", "change.json", "--plain"])
        .output()
        .expect("run closeout again");
    assert!(second.status.success());
    let second_manifest = manifest_json(&project);
    let second_targets = second_manifest["closeout"]["missing_targets"]
        .as_array()
        .expect("missing targets");
    assert_eq!(first_targets, second_targets);
}

#[test]
fn closeout_supports_git_uncommitted_and_since_sources() {
    let project = temp_project("closeout-git");
    write(&project.join("README.md"), "Document OLD_ENV.\n");
    write(
        &project.join("src").join("app.rs"),
        "std::env::var(\"OLD_ENV\").unwrap();\n",
    );
    git(&project, &["init"]);
    git(&project, &["config", "user.email", "test@example.invalid"]);
    git(&project, &["config", "user.name", "Test User"]);
    git(&project, &["add", "."]);
    git(&project, &["commit", "-m", "initial"]);

    write(
        &project.join("src").join("app.rs"),
        "std::env::var(\"NEW_ENV\").unwrap();\n",
    );
    write(
        &project.join("src").join("new-file.sh"),
        "echo $UNTRACKED_ENV\n",
    );
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
      "removed": ["let old = std::env::var(\"OLD_ENV\").unwrap();"],
      "added": ["let new = std::env::var(\"NEW_ENV\").unwrap();"]
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
        "--config-flag",
        "--code-flag",
    ] {
        assert!(new_tokens.iter().any(|value| value == token), "{token}");
    }
    for token in [
        "CONFIG_ENV",
        "CODE_ENV",
        "self.method",
        "std.fs",
        "service.timeout",
    ] {
        assert!(!new_tokens.iter().any(|value| value == token), "{token}");
    }
    let low_confidence = closeout["low_confidence_tokens"]
        .as_array()
        .expect("low confidence tokens");
    for token in ["CONFIG_ENV", "CODE_ENV"] {
        assert!(
            low_confidence
                .iter()
                .any(|value| value["token"].as_str() == Some(token)),
            "{token}"
        );
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
      "added": ["let new = std::env::var(\"NEW_ENV\").unwrap();"]
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
      "added": ["let new = std::env::var(\"NEW_ENV\").unwrap();"]
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
fn verify_matches_tokens_on_explicit_word_boundaries() {
    let project = temp_project("verify-token-boundaries");
    write(
        &project.join("README.md"),
        "OLD_ENV_EXTRA remains. NEW_ENV_EXTRA is not the token.\n",
    );
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
                "detail": "boundary"
            },
            "changed_files": ["src/app.rs"],
            "changed_categories": ["env"],
            "new_tokens": ["NEW_ENV"],
            "removed_tokens": ["OLD_ENV"],
            "missing_tokens": ["NEW_ENV"],
            "missing_targets": [
                {
                    "token": "NEW_ENV",
                    "path": "README.md",
                    "lane": "Current Dev Docs"
                }
            ],
            "possible_doc_impact": [
                {
                    "token": "OLD_ENV",
                    "signal": "stale",
                    "path": "README.md",
                    "line": 1,
                    "lane": "Current Dev Docs"
                }
            ]
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

    let failed_verify = maintenance()
        .args(["verify", "--project"])
        .arg(&project)
        .arg("--plain")
        .output()
        .expect("run verify");
    assert_eq!(failed_verify.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&failed_verify.stdout);
    assert!(!stdout.contains("stale_remaining: OLD_ENV"));
    assert!(stdout.contains("missing_remaining: NEW_ENV"));

    write(
        &project.join("README.md"),
        "OLD_ENV_EXTRA remains. Document NEW_ENV.\n",
    );
    let passed_verify = maintenance()
        .args(["verify", "--project"])
        .arg(&project)
        .arg("--plain")
        .output()
        .expect("run verify again");
    assert!(
        passed_verify.status.success(),
        "verify stdout:\n{}\nverify stderr:\n{}",
        String::from_utf8_lossy(&passed_verify.stdout),
        String::from_utf8_lossy(&passed_verify.stderr)
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
