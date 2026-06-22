# Doc Maintenance（中文说明）

> English: [../README.md](../README.md)

一个轻量的 CLI 与 agent skill，用于在代码变更后保持项目文档与代码同步。它不让模型递归读完所有文档再猜，而是生成一个短小、agent 可读的 packet，把需要审阅的确切文档路径交给只读子代理，再由主 agent 做精准编辑。

## 它做什么

- 默认从 `README.md` 和 `docs/` 发现开发文档；只有传入 `--record-docs` 时才读取记录文档。
- 从 diff 中提取变更的 token（环境变量、命令行 flag、配置键），反查它们影响的文档行。
- 标记过期行（你删掉的 token 文档里还在讲）和缺失行（你新增的 token 没有任何文档覆盖）。
- 全程本地运行：不调用模型 API、不读密钥、不起 MCP Server、不跑后台服务。

## 安装

**从 Release 下载 —— 无需 Rust。** 在 [Releases](https://github.com/QianYan-Art/maintenance/releases) 页面下载对应平台的二进制，放进系统 `PATH`：

- `maintenance-windows-x64.exe`
- `maintenance-macos-x64`、`maintenance-macos-arm64`
- `maintenance-linux-x64`

macOS 和 Linux 下载后先 `chmod +x`。

**从源码安装：**

```sh
cargo install --git https://github.com/QianYan-Art/maintenance
```

## 作为 skill 使用

skill 就是 `skill/doc-maintenance/SKILL.md` 加上它驱动的 CLI。装进编码 agent 的步骤：

1. **把 `maintenance` 二进制放进 `PATH`**，让 agent 在任何项目目录都能调用。用 `maintenance --help` 验证。
2. **把 `skill/doc-maintenance/` 复制到你的 agent 的 skills 目录** —— 例如 Claude Code 或 Codex 的按用户 skills 文件夹。
3. agent 会从 `SKILL.md` 的 front matter 加载它，并在改动后需要更新文档时调用。

## 用法

```sh
maintenance init --project .                        # 写入本地配置
maintenance route --project .                       # 接手时的读取路线
maintenance closeout --project . --git uncommitted  # 改动后的收尾
maintenance verify --project .                      # 确认编辑闭环
```

`init` 会写入 `.doc-maintenance/config.toml`，记录默认的 `dev_docs`、`record_docs`、`topic`。命令行显式参数始终优先于配置。`change-manifest` 的 JSON 格式、配置字段和 pack 兜底详见 [zh/usage.md](zh/usage.md)。

## 改动来源

`closeout` 必须有一种带内容的来源 —— 它绝不猜本轮改了什么：

- `--git uncommitted`
- `--since <git-ref>`
- `--change-manifest <path>`

故意不支持纯路径文件列表：没有行内容就无法提取 token、也无法发现过期文档。

## 它不会做什么

不新增 MCP Server、不调用模型 API、不读密钥、不写外部记忆工具、不修改任何 `archived/` 路径下的内容。生成的 packet 和本地配置都在 `.doc-maintenance/` 下，不进 Git。

## 许可证

MIT。
