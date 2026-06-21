use std::fs;

#[test]
fn readme_contains_install_and_workflow_contract() {
    let readme = fs::read_to_string("README.md").expect("read README");

    for required in [
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
    ] {
        assert!(
            readme.contains(required),
            "missing README phrase: {required}"
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
