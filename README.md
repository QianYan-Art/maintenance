# Doc Maintenance

Doc Maintenance is a lightweight Codex skill plus Rust CLI for keeping project documentation aligned with code changes. It generates small, Codex-readable packets, asks a read-only subagent to review only the listed document paths, then lets the main Codex session make precise edits.

## What It Does

- Discovers current development docs from `README.md` and `docs/` by default.
- Reads KBase or record docs only when `--record-docs` is explicitly provided.
- Builds `route`, `closeout`, and `verify` packets without calling model APIs, reading secrets, or starting an MCP server.
- Keeps generated runs and local config under `.doc-maintenance/`, which is ignored by Git.

## Install

```powershell
cargo build --release
.\scripts\copy-release.ps1
```

The copy script writes only to `codex-skill/doc-maintenance/bin/maintenance.exe`. Install or sync the skill into your own `<user-skills-dir>` only after reviewing the generated package.

## Usage

```powershell
cargo run -- init --project . --plain
cargo run -- route --project . --plain
cargo run -- closeout --project . --git uncommitted --plain
cargo run -- verify --project . --plain
```

`init` creates `.doc-maintenance/config.toml` with local defaults for `dev_docs`, `record_docs`, `summary_source`, and `topic`. Empty fields keep the default behavior. Explicit CLI flags such as `--dev-docs`, `--record-docs`, `--summary-source`, and `--topic` override config values.

## Change Sources

`closeout` requires one content-bearing source:

- `--git uncommitted`
- `--since <git-ref>`
- `--change-manifest <path>`

Pure path-only changed-files input is intentionally unsupported.

## Boundaries

- No MCP server.
- No model API calls.
- No API key or secret reading.
- No automatic writes to external memory tools.
- No automatic global skill overwrite.
- No edits to paths containing `archived`.

## Release Boundary

Publish the Git-tracked source package, not a raw local workspace archive. Do not include ignored local or generated directories such as `.mission/`, `.doc-maintenance/`, `.serena/`, or `target/`.

## Development

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Development docs are kept in `docs/`:

- Usage: [docs/usage.md](docs/usage.md)
- ADR: [docs/adr/20260621-doc-maintenance-skill-cli.md](docs/adr/20260621-doc-maintenance-skill-cli.md)

License: MIT.

---

# Doc Maintenance（中文）

Doc Maintenance 是一个轻量 Codex skill + Rust CLI，用于在项目代码变更后维护开发文档。它先生成短小的 Codex 可读 packet，再让只读子代理审阅列出的文档路径，最后由主 Codex 精准编辑。

## 项目定位

- 默认从 `README.md` 和 `docs/` 发现当前开发文档。
- 只有显式传入 `--record-docs` 时才读取 KBase 或记录文档。
- `route`、`closeout`、`verify` 全部是确定性本地命令，不调用模型 API、不读取密钥、不启动 MCP Server。
- 运行输出和本地配置都放在 `.doc-maintenance/`，并被 Git 忽略。

## 安装

```powershell
cargo build --release
.\scripts\copy-release.ps1
```

复制脚本只写入 `codex-skill/doc-maintenance/bin/maintenance.exe`。如需安装到全局 skill 目录，请先审阅生成的 skill 包，再同步到自己的 `<user-skills-dir>`。

## 用法

```powershell
cargo run -- init --project . --plain
cargo run -- route --project . --plain
cargo run -- closeout --project . --git uncommitted --plain
cargo run -- verify --project . --plain
```

`init` 会生成 `.doc-maintenance/config.toml`，可记录本地默认 `dev_docs`、`record_docs`、`summary_source` 和 `topic`。字段留空时保持默认行为；命令行传入的 `--dev-docs`、`--record-docs`、`--summary-source`、`--topic` 优先于配置。

## 三类改动来源

`closeout` 必须传入一种带内容来源：

- `--git uncommitted`
- `--since <git-ref>`
- `--change-manifest <path>`

工具故意不支持纯路径 changed-files 输入。

## 禁止事项

- 不新增 MCP Server。
- 不调用模型 API。
- 不读取 API key 或密钥。
- 不自动写外部记忆工具。
- 不自动覆盖全局 skill。
- 不读取或修改包含 `archived` 的路径。

## 发布边界

开源发布应使用 Git 跟踪的源码包，不要直接打包本地工作区。不要包含被忽略的本地过程、运行或构建目录，例如 `.mission/`、`.doc-maintenance/`、`.serena/`、`target/`。

开发文档集中维护在 `docs/`：

- 使用说明：[docs/usage.md](docs/usage.md)
- 架构决策：[docs/adr/20260621-doc-maintenance-skill-cli.md](docs/adr/20260621-doc-maintenance-skill-cli.md)

许可证：MIT。
