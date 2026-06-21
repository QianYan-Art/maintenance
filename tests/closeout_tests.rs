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
