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
- 直接点名的记录文档文件总是候选，不受 `--topic` 影响。`--topic` 只筛选记录文档目录展开出的文件：每个 topic 值按空白和逗号（含全角 `，`、`、`）拆成多个词，文件名包含任一词即通过，不区分大小写。
- 显式路径不存在，或传了记录文档却没有任何候选时，`manifest.json` 写入 `input_warnings`（`missing_path` / `no_record_docs`），packet 显示 `Input Warnings` 一节，终端同步打印告警。新建的记录文档要先创建再跑 `closeout`。
- 路径任一段等于 `archived` 时只列、不读、不改。
- `--plain` 与 `--no-banner` 去除终端装饰；agent 或脚本消费输出时用 `--plain`。
- `--project` 必须是已存在的目录 —— 只能是工作项目本身，绝不能指向记录文档库。
- 所有产物都在 `<project>/.doc-maintenance/` 下；CLI 会在其中写入自我忽略的 `.gitignore`，该目录不会进入 Git。

## 改动来源

`closeout` 不接受纯路径文件列表，必须传一种带内容来源：

```sh
maintenance closeout --project . --git uncommitted
maintenance closeout --project . --since HEAD~1
maintenance closeout --project . --change-manifest ./change.json
maintenance closeout --project . --compare ./backup/app.toml.bak ./app.toml
```

`--compare <改前> <改后>` 直接对比一对文件（通常是备份与修改后的文件），适合 Git 之外的改动，可重复传入多对。只在改后文件出现的行算新增、只在改前文件出现的行算删除（仅调整顺序不算改动），改动文件按改后路径记录。改前文件不存在表示新建，改后文件不存在表示删除。

`change-manifest` 最小 JSON（`maintenance closeout --help` 里也有）：

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

## 阅读 packet

- 同一 token 在同一文档里的多次命中合并为一行并列出行号，例如 `update` `--verbose` at `README.md` lines 1, 2, 3；`manifest.json` 仍保留每一处命中。
- `New tokens` 拆成 `already documented`（已有候选文档提到）和 `not yet documented`（即 `missing` 义务）两组。
- 记录文档里的过时命中标为 `stale (advisory)`。

## verify 语义

- 删除的 token 仍出现在**开发文档**中，记为 `stale_remaining`，`verify` 失败。
- 删除的 token 仍出现在**记录文档**中，记为 `stale_advisory`：会打印并写入 `outcome.json`，但 `verify` 仍通过。记录文档是历史，“旧键已删除”这类说明必须允许写出旧键。
- `missing` token 必须出现在目标文档（或任一候选文档）中，`verify` 才通过。

## 豁免与运行报表

提取出的 token 带置信度：有使用证据的 token（env 访问模式、`.env` 赋值、compose `environment` 键、flag、config key）为高置信，产生 `missing`/`stale` 义务；裸大写词只进低置信参考区。从测试或 fixture 路径（`tests/`、`test/`、`__tests__/`、`spec/`、`fixtures/`、`testdata/`，或 `*_test.*`、`*.test.*`、`*.spec.*`、`test_*` 这类文件名）提取的 token 一律为低置信，证据记为 `path:test`，因为测试里的 flag 和键是输入，不是用户可见的接口。

高置信 token 仍是噪声时，用豁免代替往文档里硬塞：

```sh
maintenance waive --project . SOME_TOKEN --reason "生成的常量，非用户可见"
maintenance waive --project . SOME_TOKEN --reason "内部键" --scope project --expires 2026-12-31
```

- `--scope run`（默认）把豁免绑定到最近一次 closeout run，只影响该 run 的 `verify`；需要已有 closeout run。
- `--scope project` 对所有 run 生效。建议加 `--expires YYYY-MM-DD`（含当天），到期自动失效，避免永久掩盖 token。
- 旧版本写入的条目没有 `scope`，继续按项目级豁免生效。

豁免连同理由、日期、作用域、run id 与到期日写入 `.doc-maintenance/waivers.toml`；`closeout` 与 `verify` 跳过生效中的豁免 token，并在 manifest 记录为 `ignored_tokens`。绝不为了让 `verify` 通过而添加无信息量的 token 提及。

`verify` 会把 `outcome.json` 写进对应 run 目录。汇总最近的 run：

```sh
maintenance report --project . --last 5
```

每行显示 run id、改动来源、改动文件数、高/低置信 token 数、豁免与缺失数、输入告警数、过时提示数，以及 verify 结果（`passed` / `failed` / `unverified`）。`maintenance --version` 输出 CLI 版本。

## 安装到 skill 包

终端用户最简单的方式：从 Releases 下载 `doc-maintenance-skill-<平台>` 压缩包，把解压出的 `doc-maintenance/` 目录放进 agent 的 skills 目录；二进制就在 `bin/` 里。把它加进 `PATH` 是可选项，自动安装代理修改 PATH 前应先征得同意。下面是从源码构建该包的步骤。

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
