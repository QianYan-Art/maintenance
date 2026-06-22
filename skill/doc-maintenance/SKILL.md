---
name: doc-maintenance
description: Syncs project dev docs (and explicitly named record docs) with code changes via a small packet and a read-only subagent review — without reading every doc. Use after a change when docs may be stale.
---

# Doc Maintenance

Run the bundled CLI to produce a short packet, have a read-only subagent review the candidate documents, then make precise edits based only on the `path:line` evidence it returns.

## Locate the binary

Before running a command, resolve the path to the `maintenance` executable, in order:

1. **Prefer a full path.** If you can determine this skill's own directory, use the `bin/maintenance` (`bin\maintenance.exe` on Windows) inside it.
2. **Check PATH.** Run `where maintenance` (Windows) or `command -v maintenance` (macOS/Linux); if found, call `maintenance` directly.
3. **Otherwise ask the user** for the binary's location.

Call it by the resolved path, e.g. `<path> closeout --project . --git uncommitted --plain`. Do not assume it is on PATH, and **do not modify PATH or any environment variable without the user's consent** — adding it to PATH is an optional convenience the user chooses. (Inside the source repo, `cargo run -- <command> --plain` also works.)

## Never

- Recursively read `docs/`, record docs, or the whole project before running the CLI.
- Add an MCP server, model API, background service, or secret config.
- Write to external memory tools.
- Read or modify any `archived` path; list it as historical reference only.

## Commands

```
maintenance init --project .                        # write local config (won't overwrite)
maintenance route --project .                       # reading route on handoff
maintenance closeout --project . --git uncommitted  # closeout after a change
maintenance verify --project .                      # confirm the edits closed the loop
```

`closeout` requires exactly one content-bearing source — `--git uncommitted`, `--since <git-ref>`, or `--change-manifest <path>`. Path-only changed-files are rejected; on a missing source, stop and handle `needs_input: changed_source`.

## Flow

1. Run `closeout`. The packet lists candidate paths and hit reasons only — it never inlines document bodies; `manifest.json` is the single source of truth.
2. Hand `subagent-prompt.md` to a read-only subagent. It edits nothing and returns `path:line` evidence in three kinds: `stale` (now outdated), `update` (needs change), `missing` (needs adding) — each with the matched token.
3. Read only those lines and edit the dev docs (or explicitly named record docs). `--record-docs` has no default; record docs are touched only when named.
4. Run `verify`; if any `stale` or `missing` remain, fix and re-run.

## Fallback

Only if a subagent is unavailable: `maintenance closeout --project . --git uncommitted --pack --max-lines 200`. The resulting `pack.md` is a bounded crutch, not a source of truth; you must still edit the docs and run `verify`.
