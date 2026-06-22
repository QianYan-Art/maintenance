---
name: doc-maintenance
description: Use after project code changes when the agent must update project development docs or explicitly named record docs without recursively reading every doc first.
---

# Doc Maintenance

先运行内置 CLI 生成短 packet，再让只读子代理审阅候选文档；主 agent 只根据子代理给出的 `path:line` 证据编辑文档。

## 禁止

- 禁止在运行 CLI 前递归读取 `docs/`、记录文档或整棵项目文档。
- 禁止新增 MCP Server、模型 API、后台服务或密钥配置。
- 禁止自动写外部记忆工具。
- 禁止读取或修改任意 `archived` 路径；它只能被列为历史参考。

## 命令

仓库源码运行：

```powershell
cargo run -- <command> --plain
```

安装包运行：

```powershell
bin\maintenance.exe <command> --plain
```

macOS/Linux 安装包运行：

```bash
./bin/maintenance <command> --plain
```

最小调用：

```powershell
cargo run -- init --project . --plain
cargo run -- route --project . --plain
cargo run -- closeout --project . --git uncommitted --plain
cargo run -- verify --project . --plain
```

`init` 只生成本地 `.doc-maintenance/config.toml`，不会覆盖已存在配置。配置可写默认 `dev_docs`、`record_docs`、`topic`；字段留空时沿用自动发现开发文档、不默认读取记录文档的规则。命令行显式传入的路径和 topic 优先于配置。

`closeout` 必须且只能使用一种带内容改动来源：

- `--git uncommitted`
- `--since <git-ref>`
- `--change-manifest <path>`

不要使用纯路径 changed-files；缺少改动来源时停止该流程并处理 `needs_input: changed_source`。

## 流程

1. 首次在项目使用时可运行 `init` 写入本地默认配置。
2. 任务开始或接手时运行 `route`，只读取生成的 `packet.md`。
3. 完成代码改动后运行 `closeout`。
4. 把生成的 `subagent-prompt.md` 交给只读子代理。
5. 子代理只读候选路径，不编辑文件，并按三类返回：
   - `stale`: 已过期内容，必须带 `path:line` 和命中 token。
   - `update`: 需更新内容，必须带 `path:line` 和命中 token。
   - `missing`: 需新增内容，必须给出目标路径和命中 token。
6. 主 agent 只读取子代理返回的必要 `path:line` 片段并编辑开发文档或显式点名的记录文档。
7. 编辑完成后运行 `verify`；若仍有 stale 或 missing，继续修文档并重跑 `verify`。

## Packet 规则

- `packet.md` 只列候选路径、lane、命中原因、changed files、tokens 和 possible doc impact。
- `packet.md` 不内联文档正文。
- `manifest.json` 是生成文件的单一数据源。
- `--record-docs` 无默认值；只有使用者或当前任务显式点名时才处理记录文档。

## Fallback

只有子代理不可用时，才允许运行：

```powershell
cargo run -- closeout --project . --git uncommitted --pack --max-lines 200 --plain
```

`pack.md` 是限额兜底材料，不是长期事实源；读完仍必须编辑目标文档并运行 `verify`。
