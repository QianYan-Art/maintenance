use std::fs;
use std::path::Path;

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
        "## Usage",
        "## Change Sources",
        "## Boundaries",
        "## Release Boundary",
        "# Doc Maintenance（中文）",
        "## 项目定位",
        "## 安装",
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
        "Git 跟踪的源码包",
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
        "KBase 记录文档无默认值",
        "--git uncommitted",
        "--since <git-ref>",
        "--change-manifest <path>",
        "verify",
        "--pack --max-lines 200",
        "cargo build --release",
        ".\\scripts\\copy-release.ps1",
        "不自动覆盖",
        "<user-skills-dir>",
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
fn repository_contains_no_private_paths_or_personal_names() {
    let mut files = Vec::new();
    collect_files(Path::new("."), &mut files);
    let private_kbase = ["QianYan", "KBase"].join("-");
    let personal_name = "\u{963f}\u{989c}".to_string();
    let forbidden = vec![
        format!("C:{}Users{}<user-dir>", '\\', '\\'),
        ["C:", "Users", "<user-dir>"].join("/"),
        format!("D:{}Answer{}{}", '\\', '\\', private_kbase),
        format!("D:/Answer/{private_kbase}"),
        private_kbase,
        personal_name,
    ];

    for path in files {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for term in &forbidden {
            assert!(
                !text.contains(term),
                "private term {term:?} found in {}",
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
