---
name: doc-maintenance
description: Use after project code changes when Codex must update project development docs or explicitly named KBase records without recursively reading every doc first.
---

# Doc Maintenance

调用内置 Rust CLI 生成文档收尾路线，再由主 Codex 精准编辑项目开发文档。

## 内置工具

- 仓库源码运行：`cargo run -- <command> --plain`
- 安装包路径：`bin/maintenance.exe`

## 第一版命令

- `route`：任务开始或接手时生成读取路线。
- `closeout`：代码改动完成后，基于带内容的改动来源生成收尾 packet。
- `verify`：主 Codex 编辑文档后验证 stale token 已消失、missing token 已出现。

## 硬规则

- 不新增 MCP Server。
- CLI 不调用模型、不读取 API key、不写 nowledge-mem。
- `--record-docs` 无默认值；只有阿颜或当前任务显式点名时才处理 KBase 记录文档。
- 路径任一段为 `archived` 时只列为历史参考，不读、不改。
- 生成的 `packet.md`、`subagent-prompt.md`、`manifest.json` 是一次性材料，默认写入 `.doc-maintenance/runs/`，不作为长期事实源。

## 基础用法

```powershell
cargo run -- route --project . --plain
cargo run -- closeout --project . --git uncommitted --plain
cargo run -- verify --project . --plain
```
