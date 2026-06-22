# Doc Maintenance

`doc-maintenance` 是一个 Codex skill + Rust CLI，用于在项目改动后生成文档收尾路线。

## 快速开始

```powershell
cargo run -- init --project . --plain
cargo run -- route --project . --plain
cargo run -- closeout --project . --git uncommitted --plain
cargo run -- verify --project . --plain
```

`init` 会生成本地 `.doc-maintenance/config.toml`，用于记录默认 `dev_docs`、`record_docs` 和 `topic`。配置留空时保持当前默认：自动发现 `README.md` 与 `docs/`，不默认读取 KBase；命令行传入的 `--dev-docs`、`--record-docs`、`--topic` 会覆盖配置。

开发文档集中维护在 `docs/`：

- 使用说明：[docs/usage.md](docs/usage.md)
- 架构决策：[docs/adr/20260621-doc-maintenance-skill-cli.md](docs/adr/20260621-doc-maintenance-skill-cli.md)
