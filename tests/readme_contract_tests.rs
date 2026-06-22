use std::fs;
use std::path::Path;

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
        "maintenance-linux-x64",
        "maintenance-macos-x64",
        "maintenance-macos-arm64",
        "maintenance-windows-x64.exe",
        "gh release create",
        "gh release upload",
        "GH_REPO: ${{ github.repository }}",
    ] {
        assert!(
            release.contains(required),
            "missing release workflow phrase: {required}"
        );
    }
}

#[test]
fn readme_points_to_docs_directory() {
    let readme = fs::read_to_string("README.md").expect("read README");

    assert!(readme.contains("开发文档集中维护在 `docs/`"));
    assert!(readme.contains("docs/usage.md"));
    assert!(readme.contains("docs/adr/20260621-doc-maintenance-skill-cli.md"));
}

#[test]
fn readme_contains_public_bilingual_contract() {
    let readme = fs::read_to_string("README.md").expect("read README");

    for required in [
        "## What It Does",
        "## Install",
        "GitHub Releases",
        "maintenance-windows-x64.exe",
        "maintenance-macos-x64",
        "maintenance-macos-arm64",
        "maintenance-linux-x64",
        "cargo install --git https://github.com/QianYan-Art/maintenance",
        "Claude Code",
        "~/.claude/skills/doc-maintenance/",
        "~/.codex/skills/doc-maintenance/",
        "opencode",
        "pi",
        "To be verified from tool docs or contributed by the community",
        "## Usage",
        "## Change Sources",
        "## Boundaries",
        "## Release Boundary",
        "# Doc Maintenance（中文）",
        "## 项目定位",
        "## 安装",
        "从 GitHub Releases 下载预编译二进制",
        "## 用法",
        "## 三类改动来源",
        "## 禁止事项",
        "## 发布边界",
        "cargo run -- init --project . --plain",
        "cargo run -- route --project . --plain",
        "cargo run -- closeout --project . --git uncommitted --plain",
        "cargo run -- verify --project . --plain",
        "--since <git-ref>",
        "--change-manifest <path>",
        "No MCP server",
        "不新增 MCP Server",
        "Git-tracked source package",
        "Compiled binaries are attached to GitHub Releases",
        "skill/doc-maintenance/bin/.gitkeep",
        "skill/doc-maintenance/bin/maintenance.exe",
        "Git 跟踪的源码包",
        "二进制由 `v*` tag workflow 上传到 GitHub Releases",
        "本机 `copy-release` 输出仅本地保留",
        ".mission/",
        ".doc-maintenance/",
        ".serena/",
        "target/",
        "MIT",
    ] {
        assert!(
            readme.contains(required),
            "missing README phrase: {required}"
        );
    }
}

#[test]
fn repository_has_mit_license() {
    let license = fs::read_to_string("LICENSE").expect("read LICENSE");

    assert!(license.contains("MIT License"));
    assert!(license.contains("Permission is hereby granted"));
}

#[test]
fn docs_usage_contains_install_and_workflow_contract() {
    let usage = fs::read_to_string("docs/usage.md").expect("read docs usage");

    for required in [
        "init",
        ".doc-maintenance/config.toml",
        "默认发现",
        "记录文档无默认值",
        "--git uncommitted",
        "--since <git-ref>",
        "--change-manifest <path>",
        "verify",
        "--pack --max-lines 200",
        "cargo build --release",
        ".\\scripts\\copy-release.ps1",
        "./scripts/copy-release.sh",
        "不自动覆盖",
        "skill/doc-maintenance/bin/",
        ".doc-maintenance/",
        ".mission/",
        ".serena/",
        "target/",
        "开源发布应使用 Git 跟踪的源码包",
    ] {
        assert!(
            usage.contains(required),
            "missing docs usage phrase: {required}"
        );
    }
}

#[test]
fn adr_does_not_claim_path_only_changed_files_source() {
    let adr =
        fs::read_to_string("docs/adr/20260621-doc-maintenance-skill-cli.md").expect("read ADR");

    assert!(adr.contains("git uncommitted"));
    assert!(adr.contains("--since <git-ref>"));
    assert!(adr.contains("change-manifest"));
    assert!(!adr.contains("diff/changed-files/change-manifest"));
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
