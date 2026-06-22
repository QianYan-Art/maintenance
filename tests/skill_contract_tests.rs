use std::fs;
use std::path::Path;

const SKILL_PATH: &str = "skill/doc-maintenance/SKILL.md";

#[test]
fn skill_contract_contains_required_workflow_rules() {
    let skill = fs::read_to_string(SKILL_PATH).expect("read skill");

    for required in [
        "before running the CLI",
        "maintenance closeout --project . --git uncommitted",
        "--git uncommitted",
        "--since <git-ref>",
        "--change-manifest <path>",
        "subagent-prompt.md",
        "`stale`",
        "`update`",
        "`missing`",
        "path:line",
        "verify",
        "--pack --max-lines 200",
        "never inlines document bodies",
    ] {
        assert!(
            skill.contains(required),
            "missing contract phrase: {required}"
        );
    }
}

#[test]
fn skill_contract_locates_binary_without_touching_env() {
    let skill = fs::read_to_string(SKILL_PATH).expect("read skill");

    for required in [
        "Locate the binary",
        "Prefer a full path",
        "ask the user",
        "do not modify PATH or any environment variable without the user's consent",
    ] {
        assert!(
            skill.contains(required),
            "missing binary-resolution rule: {required}"
        );
    }
}

#[test]
fn skill_contract_does_not_invite_heavy_or_unsafe_integrations() {
    let skill = fs::read_to_string(SKILL_PATH).expect("read skill");

    for required in ["MCP server", "model API", "external memory tools"] {
        assert!(skill.contains(required), "missing prohibition: {required}");
    }
    assert!(!skill.contains("API key"));
}

#[test]
fn skill_package_uses_neutral_layout_and_wording() {
    let skill = fs::read_to_string(SKILL_PATH).expect("read skill");

    assert!(skill.contains("name: doc-maintenance"));
    assert!(skill.contains("description: Syncs project dev docs"));

    let codex = ['C', 'o', 'd', 'e', 'x'].iter().collect::<String>();
    let old_record_label = ['K', 'B', 'a', 's', 'e'].iter().collect::<String>();
    let old_memory_tool = ["nowledge", "-mem"].concat();
    let old_skill_dir = ["codex", "-skill"].concat();
    assert!(!skill.contains(&codex));
    assert!(!skill.contains(&old_record_label));
    assert!(!skill.contains(&old_memory_tool));
    assert!(Path::new("skill/doc-maintenance/bin/.gitkeep").exists());
    assert!(!Path::new(&old_skill_dir).exists());
}

#[test]
fn repository_contains_no_old_skill_path_or_record_label() {
    let mut files = Vec::new();
    collect_files(Path::new("."), &mut files);
    let old_skill_path = ["codex", "-skill"].concat();
    let old_record_label = ['K', 'B', 'a', 's', 'e'].iter().collect::<String>();

    for path in files {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        assert!(
            !text.contains(&old_skill_path),
            "old skill path found in {}",
            path.display()
        );
        assert!(
            !text.contains(&old_record_label),
            "old record label found in {}",
            path.display()
        );
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
