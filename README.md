# Doc Maintenance

`doc-maintenance` 是一个 Codex skill + Rust CLI，用于在项目改动后生成文档收尾路线。它不替 Codex 自动写文档；它先生成短 packet，再让只读子代理审阅候选文档，最后由主 Codex 精准编辑项目开发文档或显式点名的 KBase 记录。

## 定位

- 运行时轻量：不做 MCP Server、不接模型 API、不读密钥、不写 nowledge-mem。
- 项目开发文档默认发现：存在的 `README.md` 和 `docs/`。
- KBase 记录文档无默认值：只有显式传 `--record-docs` 时才处理点名路径。
- `archived` 路径只列为历史参考，不读、不改。

## 常用命令

```powershell
cargo run -- route --project . --plain
cargo run -- closeout --project . --git uncommitted --plain
cargo run -- verify --project . --plain
```

`closeout` 必须使用一种带内容改动来源：

- `--git uncommitted`
- `--since <git-ref>`
- `--change-manifest <path>`

子代理不可用时才使用限额 pack：

```powershell
cargo run -- closeout --project . --git uncommitted --pack --max-lines 200 --plain
```

## 安装到 skill 包

先构建 release：

```powershell
cargo build --release
```

再复制到仓库内 skill 包：

```powershell
.\scripts\copy-release.ps1
```

脚本只复制到 `codex-skill/doc-maintenance/bin/maintenance.exe`，不自动覆盖 `<user-dir>\.codex\skills`。全局安装前请先确认当前源码和 release 构建一致。

## 验证

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

更多说明见 `docs/usage.md`。
