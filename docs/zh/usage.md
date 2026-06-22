# Doc Maintenance — 使用说明

`doc-maintenance` 是一个 CLI 加 agent skill。它不替你写文档：先生成短 packet，让只读子代理审阅候选文档，再由主 agent 精准编辑开发文档（或显式点名的记录文档）。

## 流程

1. `init` —— 写入本地 `.doc-maintenance/config.toml`（不覆盖已有配置）。
2. `route` —— 任务开始或接手时生成读取路线。
3. `closeout` —— 改动后，从带内容的改动来源生成 `packet.md`、`subagent-prompt.md`、`manifest.json`。
4. 只读子代理读取 `subagent-prompt.md` 里的候选路径，按 `stale`/`update`/`missing` 返回 `path:line` 证据。
5. 主 agent 只读这些行并编辑文档。
6. `verify` —— 确认删除的 token 已从文档消失、新增的 token 已出现。

## 输入与默认

- `--project` 默认当前目录。
- `.doc-maintenance/config.toml` 保存 `dev_docs`、`record_docs`、`summary_source`、`topic` 的默认值。字段留空时保持默认：自动发现开发文档、不碰记录文档。
- 命令行 `--dev-docs`、`--record-docs`、`--summary-source`、`--topic` 优先于配置。
- 未传 `--dev-docs` 时自动发现存在的 `README.md` 和 `docs/`。
- `--record-docs` 无默认值；记录文档必须人工点名。
- 路径任一段等于 `archived` 时只列、不读、不改。

## 改动来源

`closeout` 不接受纯路径文件列表，必须传一种带内容来源：

```sh
maintenance closeout --project . --git uncommitted
maintenance closeout --project . --since HEAD~1
maintenance closeout --project . --change-manifest ./change.json
```

`change-manifest` 最小 JSON：

```json
{
  "files": [
    {
      "path": "src/app.rs",
      "added": ["let key = \"NEW_ENV\";"],
      "removed": ["let key = \"OLD_ENV\";"]
    }
  ]
}
```

## Pack 兜底

只有子代理不可用时：

```sh
maintenance closeout --project . --git uncommitted --pack --max-lines 200
```

`pack.md` 只含候选路径、token、命中行与少量上下文，受 `--max-lines` 限制，不是长期事实源。

## 安装到 skill 包

```sh
cargo build --release
./scripts/copy-release.sh   # Windows 用 .\scripts\copy-release.ps1
```

脚本只复制到 `skill/doc-maintenance/bin/`，不覆盖任何全局 skills 目录。全局安装前确认源码与 release 构建一致。

## 验证构建

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```
