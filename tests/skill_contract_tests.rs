use std::fs;

const SKILL_PATH: &str = "codex-skill/doc-maintenance/SKILL.md";

#[test]
fn skill_contract_contains_required_workflow_rules() {
    let skill = fs::read_to_string(SKILL_PATH).expect("read skill");

    for required in [
        "禁止在运行 CLI 前递归读取",
        "closeout --project . --git uncommitted",
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

    for forbidden in ["禁止新增 MCP Server", "模型 API", "禁止自动写 nowledge-mem"] {
        assert!(
            skill.contains(forbidden),
            "missing prohibition: {forbidden}"
        );
    }
    assert!(!skill.contains("读取 API key"));
}
