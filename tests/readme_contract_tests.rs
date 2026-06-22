use std::fs;
use std::path::Path;
use std::process::Command;

use regex::Regex;

#[test]
fn github_workflows_cover_ci_and_release_contract() {
    let ci = fs::read_to_string(".github/workflows/ci.yml").expect("read CI workflow");
    let release =
        fs::read_to_string(".github/workflows/release.yml").expect("read release workflow");

    serde_yaml::from_str::<serde_yaml::Value>(&ci).expect("parse CI workflow YAML");
    serde_yaml::from_str::<serde_yaml::Value>(&release).expect("parse release workflow YAML");

    for required in [
        "cargo fmt --all --check",
        "cargo clippy --all-targets -- -D warnings",
        "cargo test",
    ] {
        assert!(ci.contains(required), "missing CI command: {required}");
    }

    for required in [
        "tags:",
        "\"v*\"",
        "x86_64-unknown-linux-gnu",
        "macos-15-intel",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "doc-maintenance-skill-linux-x64.tar.gz",
        "doc-maintenance-skill-macos-x64.tar.gz",
        "doc-maintenance-skill-macos-arm64.tar.gz",
        "doc-maintenance-skill-windows-x64.zip",
        "bundle-root/doc-maintenance",
        "skill/doc-maintenance/SKILL.md",
        "$bundle_root/bin/maintenance",
        "tar -czf",
        "Compress-Archive",
        "gh release create",
        "gh release upload",
        "GH_REPO: ${{ github.repository }}",
    ] {
        assert!(
            release.contains(required),
            "missing release workflow phrase: {required}"
        );
    }

    for forbidden in [
        "asset_name:",
        "maintenance-linux-x64",
        "maintenance-macos-x64",
        "maintenance-macos-arm64",
        "maintenance-windows-x64.exe",
    ] {
        assert!(
            !release.contains(forbidden),
            "release workflow should not upload bare executable asset: {forbidden}"
        );
    }
}

#[test]
fn readme_points_to_docs_directory() {
    let readme = fs::read_to_string("README.md").expect("read README");
    let zh_readme = fs::read_to_string("docs/README.zh-CN.md").expect("read zh README");

    assert!(readme.contains("docs/en/usage.md"));
    assert!(readme.contains("docs/README.zh-CN.md"));
    assert!(!readme.contains("docs/usage.md"));
    assert!(!readme.contains("docs/adr/"));
    assert!(zh_readme.contains("zh/usage.md"));
}

#[test]
fn readme_contains_public_bilingual_contract() {
    let readme = fs::read_to_string("README.md").expect("read README");

    for required in [
        "## What it does",
        "## Install",
        "doc-maintenance-skill-windows-x64.zip",
        "doc-maintenance-skill-macos-x64.tar.gz",
        "doc-maintenance-skill-macos-arm64.tar.gz",
        "doc-maintenance-skill-linux-x64.tar.gz",
        "cargo install --git https://github.com/QianYan-Art/maintenance",
        "## Use it as a skill",
        "maintenance --help",
        "skill/doc-maintenance/",
        "## Usage",
        "maintenance init --project .",
        "maintenance route --project .",
        "maintenance closeout --project . --git uncommitted",
        "maintenance verify --project .",
        "## Change sources",
        "--since <git-ref>",
        "--change-manifest <path>",
        "## What it won't do",
        "no model API calls",
        "no secret reading",
        "docs/README.zh-CN.md",
        "docs/en/usage.md",
        "MIT",
        "automated installers should ask before modifying",
        "call it by its full path",
    ] {
        assert!(
            readme.contains(required),
            "missing README phrase: {required}"
        );
    }

    assert!(!readme.contains("cargo run --"));
    assert!(!readme.contains("~/.claude/skills/"));
    assert!(!readme.contains("opencode"));
    assert!(!readme.contains("## Boundaries"));
    assert!(!readme.contains("## Release Boundary"));
    assert!(!readme.contains("must be on your"));
    assert!(!readme.contains("Grab the binary for your platform"));
}

#[test]
fn repository_has_mit_license() {
    let license = fs::read_to_string("LICENSE").expect("read LICENSE");

    assert!(license.contains("MIT License"));
    assert!(license.contains("Permission is hereby granted"));
}

#[test]
fn docs_usage_contains_install_and_workflow_contract() {
    let en = fs::read_to_string("docs/en/usage.md").expect("read English usage");
    let zh = fs::read_to_string("docs/zh/usage.md").expect("read Chinese usage");

    for required in [
        "# Doc Maintenance — Usage",
        "## Workflow",
        "## Change sources",
        "maintenance closeout --project . --git uncommitted",
        "maintenance closeout --project . --since HEAD~1",
        "maintenance closeout --project . --change-manifest ./change.json",
        "## Pack fallback",
        "maintenance closeout --project . --git uncommitted --pack --max-lines 200",
        "## Install into a skill package",
        "./scripts/copy-release.sh",
        "skill/doc-maintenance/bin/",
        "## Verify the build",
        "cargo clippy --all-targets -- -D warnings",
        "automated installers should ask before modifying",
    ] {
        assert!(
            en.contains(required),
            "missing English usage phrase: {required}"
        );
    }

    for required in [
        "# Doc Maintenance — 使用说明",
        "## 流程",
        ".doc-maintenance/config.toml",
        "## 改动来源",
        "maintenance closeout --project . --git uncommitted",
        "maintenance closeout --project . --since HEAD~1",
        "maintenance closeout --project . --change-manifest ./change.json",
        "## Pack 兜底",
        "maintenance closeout --project . --git uncommitted --pack --max-lines 200",
        "## 安装到 skill 包",
        "./scripts/copy-release.sh",
        "skill/doc-maintenance/bin/",
        "## 验证构建",
        "cargo clippy --all-targets -- -D warnings",
        "自动安装代理修改 PATH 前应先征得同意",
    ] {
        assert!(
            zh.contains(required),
            "missing Chinese usage phrase: {required}"
        );
    }
}

#[test]
fn git_tracks_public_docs_only() {
    let output = Command::new("git")
        .args(["ls-files"])
        .output()
        .expect("run git ls-files");

    assert!(output.status.success());
    let files = String::from_utf8(output.stdout).expect("git ls-files utf8");

    for forbidden in ["CONTEXT.md", "docs/adr/", "docs/usage.md", ".exe"] {
        assert!(
            !files.contains(forbidden),
            "tracked forbidden path: {forbidden}"
        );
    }
    for required in [
        "docs/en/usage.md",
        "docs/zh/usage.md",
        "docs/README.zh-CN.md",
    ] {
        assert!(
            files.contains(required),
            "missing tracked public doc: {required}"
        );
    }

    let gitignore = fs::read_to_string(".gitignore").expect("read gitignore");
    assert!(gitignore.contains("CONTEXT.md"));
    assert!(gitignore.contains("docs/adr/"));
}

#[test]
fn repository_contains_no_private_path_patterns() {
    let mut files = Vec::new();
    collect_files(Path::new("."), &mut files);
    let user_dir = ['U', 's', 'e', 'r', 's'].iter().collect::<String>();
    let private_root = ['A', 'n', 's', 'w', 'e', 'r'].iter().collect::<String>();
    let patterns = vec![
        Regex::new(&format!(
            r"(?i)[A-Z]:[\\/]+{}[\\/]+[^\\/\s`]+",
            regex::escape(&user_dir)
        ))
        .expect("user path regex"),
        Regex::new(&format!(
            r"(?i)[A-Z]:[\\/]+{}[\\/]+",
            regex::escape(&private_root)
        ))
        .expect("private root regex"),
    ];

    for path in files {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for pattern in &patterns {
            assert!(
                !pattern.is_match(&text),
                "private path pattern {:?} found in {}",
                pattern.as_str(),
                path.display()
            );
        }
    }
}

fn collect_files(path: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries {
        let entry = entry.expect("directory entry");
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if path.is_dir() {
            if matches!(
                name,
                ".git" | ".doc-maintenance" | ".mission" | ".serena" | "target"
            ) {
                continue;
            }
            collect_files(&path, files);
        } else {
            files.push(path);
        }
    }
}
