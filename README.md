# Doc Maintenance

Doc Maintenance is a lightweight coding-agent skill plus Rust CLI for keeping project documentation aligned with code changes. It generates small, agent-readable packets, asks a read-only subagent to review only the listed document paths, then lets the main agent make precise edits.

## What It Does

- Discovers current development docs from `README.md` and `docs/` by default.
- Reads record docs only when `--record-docs` is explicitly provided.
- Builds `route`, `closeout`, and `verify` packets without calling model APIs, reading secrets, or starting an MCP server.
- Keeps generated runs and local config under `.doc-maintenance/`, which is ignored by Git.

## Install

Option 1, download a prebuilt binary from GitHub Releases. This path does not require Rust:

- Windows x64: `maintenance-windows-x64.exe`
- macOS x64: `maintenance-macos-x64`
- macOS arm64: `maintenance-macos-arm64`
- Linux x64: `maintenance-linux-x64`

On macOS and Linux, run `chmod +x ./maintenance-*` after download if the executable bit is not preserved.

Option 2, build or install from source:

```powershell
cargo install --git https://github.com/QianYan-Art/maintenance
```

```powershell
cargo build --release
.\scripts\copy-release.ps1
```

On macOS and Linux, use the matching shell script:

```bash
cargo build --release
./scripts/copy-release.sh
```

The copy scripts write only to `skill/doc-maintenance/bin/`. Install or sync the skill into your own tool-specific skills directory only after reviewing the generated package.

| Tool | Install location | Status |
| --- | --- | --- |
| Claude Code | `~/.claude/skills/doc-maintenance/` | exact |
| Codex | `~/.codex/skills/doc-maintenance/` | exact |
| opencode | To be verified from tool docs or contributed by the community | not asserted |
| pi | To be verified from tool docs or contributed by the community | not asserted |

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

Publish the Git-tracked source package, not a raw local workspace archive. Do not include ignored local or generated directories such as `.mission/`, `.doc-maintenance/`, `.serena/`, or `target/`. Git tracks `skill/doc-maintenance/SKILL.md` and `skill/doc-maintenance/bin/.gitkeep`, but not `skill/doc-maintenance/bin/maintenance` or `skill/doc-maintenance/bin/maintenance.exe`. Compiled binaries are attached to GitHub Releases by the `v*` tag workflow; local `copy-release` output stays local.

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

Doc Maintenance 是一个轻量 coding-agent skill + Rust CLI，用于在项目代码变更后维护开发文档。它先生成短小的 agent 可读 packet，再让只读子代理审阅列出的文档路径，最后由主 agent 精准编辑。

## 项目定位

- 默认从 `README.md` 和 `docs/` 发现当前开发文档。
- 只有显式传入 `--record-docs` 时才读取记录文档。
- `route`、`closeout`、`verify` 全部是确定性本地命令，不调用模型 API、不读取密钥、不启动 MCP Server。
- 运行输出和本地配置都放在 `.doc-maintenance/`，并被 Git 忽略。

## 安装

路径一：从 GitHub Releases 下载预编译二进制。这条路径不需要 Rust：

- Windows x64：`maintenance-windows-x64.exe`
- macOS x64：`maintenance-macos-x64`
- macOS arm64：`maintenance-macos-arm64`
- Linux x64：`maintenance-linux-x64`

macOS 和 Linux 下载后，如执行位未保留，先运行 `chmod +x ./maintenance-*`。

路径二：从源码构建或安装：

```powershell
cargo install --git https://github.com/QianYan-Art/maintenance
```

```powershell
cargo build --release
.\scripts\copy-release.ps1
```

macOS 和 Linux 使用对应 shell 脚本：

```bash
cargo build --release
./scripts/copy-release.sh
```

复制脚本只写入 `skill/doc-maintenance/bin/`。如需安装到全局 skill 目录，请先审阅生成的 skill 包，再同步到对应工具目录。

| 工具 | 安装位置 | 状态 |
| --- | --- | --- |
| Claude Code | `~/.claude/skills/doc-maintenance/` | 确切路径 |
| Codex | `~/.codex/skills/doc-maintenance/` | 确切路径 |
| opencode | 待工具文档核实或社区补充 | 不断言 |
| pi | 待工具文档核实或社区补充 | 不断言 |

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

开源发布应使用 Git 跟踪的源码包，不要直接打包本地工作区。不要包含被忽略的本地过程、运行或构建目录，例如 `.mission/`、`.doc-maintenance/`、`.serena/`、`target/`。Git 只跟踪 `skill/doc-maintenance/SKILL.md` 与 `skill/doc-maintenance/bin/.gitkeep`，不跟踪 `skill/doc-maintenance/bin/maintenance` 或 `skill/doc-maintenance/bin/maintenance.exe`。编译后二进制由 `v*` tag workflow 上传到 GitHub Releases；本机 `copy-release` 输出仅本地保留。

开发文档集中维护在 `docs/`：

- 使用说明：[docs/usage.md](docs/usage.md)
- 架构决策：[docs/adr/20260621-doc-maintenance-skill-cli.md](docs/adr/20260621-doc-maintenance-skill-cli.md)

许可证：MIT。
