# Doc Maintenance

A lightweight CLI and agent skill that keeps project documentation in sync with code changes. Instead of letting a model recursively read every doc and guess, it builds a small, agent-readable packet, hands a read-only subagent the exact paths to review, and lets the main agent make precise edits.

> 中文说明 / Chinese guide: [docs/README.zh-CN.md](docs/README.zh-CN.md)

## What it does

- Discovers development docs from `README.md` and `docs/` by default; reads record docs only when you pass `--record-docs`.
- Extracts changed tokens — environment variables, flags, config keys — from a diff and maps them back to the doc lines they affect.
- Flags stale lines (a token you removed that a doc still mentions) and missing ones (a token you added that no doc covers).
- Runs entirely locally: no model API, no secrets, no MCP server, no background service.

## Install

**From a release — no Rust required.** Grab the binary for your platform from the [Releases](https://github.com/QianYan-Art/maintenance/releases) page and put it on your `PATH`:

- `maintenance-windows-x64.exe`
- `maintenance-macos-x64`, `maintenance-macos-arm64`
- `maintenance-linux-x64`

On macOS and Linux, run `chmod +x` on the downloaded file first.

**From source:**

```sh
cargo install --git https://github.com/QianYan-Art/maintenance
```

## Use it as a skill

The skill is `skill/doc-maintenance/SKILL.md` plus the `maintenance` CLI it drives. The skill resolves the binary by a full path (or asks you for it), so it works without changing any environment variable.

**From a release bundle (easiest):** download `doc-maintenance-skill-<platform>` from [Releases](https://github.com/QianYan-Art/maintenance/releases) and unpack it. Drop the `doc-maintenance/` folder into your agent's skills directory — for example, the per-user skills folder used by Claude Code or Codex. The binary ships with it in `bin/`.

**From source:** copy `skill/doc-maintenance/` into that skills directory and put the built binary in its `bin/`.

The agent loads it from the `SKILL.md` front matter and runs it when docs need updating after a change.

**Optional — add to PATH:** put the binary's directory on your `PATH` (verify with `maintenance --help`) so you can also call a bare `maintenance`. This is a convenience only; automated installers should ask before modifying your `PATH`.

## Usage

```sh
maintenance init --project .                        # write local config
maintenance route --project .                       # reading route on handoff
maintenance closeout --project . --git uncommitted  # closeout after changes
maintenance verify --project .                      # confirm the edits closed the loop
```

`init` writes `.doc-maintenance/config.toml` with your default `dev_docs`, `record_docs`, and `topic`. Explicit flags always override the config.

For the `change-manifest` format, config fields, and the pack fallback, see [docs/en/usage.md](docs/en/usage.md).

## Change sources

`closeout` needs one content-bearing source — it never guesses what changed:

- `--git uncommitted`
- `--since <git-ref>`
- `--change-manifest <path>`

Path-only file lists are intentionally unsupported: without line content there is no way to extract tokens or catch stale docs.

## What it won't do

No MCP server, no model API calls, no secret reading, no writes to external memory tools, and no edits to anything under an `archived/` path. Generated packets and local config live under `.doc-maintenance/` and stay out of Git.

## License

MIT.
