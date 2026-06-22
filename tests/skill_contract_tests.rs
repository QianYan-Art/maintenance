use std::fs;
use std::path::Path;

const SKILL_PATH: &str = "skill/doc-maintenance/SKILL.md";

#[test]
fn skill_contract_contains_required_workflow_rules() {
    let skill = fs::read_to_string(SKILL_PATH).expect("read skill");

    for required in [
        "禁止在运行 CLI 前递归读取",
        "maintenance <command> --plain",
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
        "packet.md` 不内联文档正文",
    ] {
        assert!(
            skill.contains(required),
            "missing contract phrase: {required}"
        );
    }
}

#[test]
fn skill_contract_does_not_invite_heavy_or_unsafe_integrations() {
    let skill = fs::read_to_string(SKILL_PATH).expect("read skill");

    for forbidden in ["禁止新增 MCP Server", "模型 API", "禁止自动写外部记忆工具"]
    {
        assert!(
            skill.contains(forbidden),
            "missing prohibition: {forbidden}"
        );
    }
    assert!(!skill.contains("读取 API key"));
}

#[test]
fn skill_package_uses_neutral_layout_and_wording() {
    let skill = fs::read_to_string(SKILL_PATH).expect("read skill");

    assert!(skill.contains("name: doc-maintenance"));
    assert!(skill.contains("description: Use after project code changes when the agent"));
    assert!(skill.contains("主 agent"));
    assert!(skill.contains("记录文档"));
    assert!(skill.contains("源码开发场景"));
    assert!(skill.contains("cargo run -- <command> --plain"));
    assert!(skill.contains("已安装 skill 运行"));
    assert!(skill.contains("`PATH`"));
    assert!(skill.contains("不要依赖相对 `bin/` 路径"));
    assert!(!skill.contains("bin\\maintenance.exe <command>"));
    assert!(!skill.contains("./bin/maintenance <command>"));
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
