# Doc Maintenance — Usage

`doc-maintenance` is a CLI plus an agent skill. It does not write docs for you: it produces a short packet, has a read-only subagent review the candidate documents, and lets the main agent make precise edits to your development docs (or explicitly named record docs).

## Workflow

1. `init` — write a local `.doc-maintenance/config.toml` (never overwrites an existing one).
2. `route` — generate a reading route when you start or pick up a task.
3. `closeout` — after a change, build `packet.md`, `subagent-prompt.md`, and `manifest.json` from a content-bearing change source.
4. A read-only subagent reads the candidate paths in `subagent-prompt.md` and returns `path:line` evidence as `stale` / `update` / `missing`.
5. The main agent reads only those lines and edits the docs.
6. `verify` — confirm removed tokens are gone from the docs and added tokens now appear.

## Inputs and defaults

- `--project` defaults to the current directory.
- `.doc-maintenance/config.toml` holds defaults for `dev_docs`, `record_docs`, `summary_source`, and `topic`. Empty fields keep the defaults: auto-discover dev docs, never touch record docs.
- Explicit `--dev-docs`, `--record-docs`, `--summary-source`, `--topic` override the config.
- With no `--dev-docs`, the tool discovers `README.md` and `docs/` if present.
- `--record-docs` has no default; record docs are opt-in.
- Any path segment equal to `archived` is listed only — never read or edited.
- `--plain` and `--no-banner` strip terminal decoration; use `--plain` when an agent or script consumes the output.
- `--project` must be an existing directory — the working project itself, never the record-docs vault.
- All artifacts live under `<project>/.doc-maintenance/`; the CLI writes a self-ignoring `.gitignore` inside it, so the directory stays out of Git.

## Change sources

`closeout` rejects path-only file lists. Pass exactly one content-bearing source:

```sh
maintenance closeout --project . --git uncommitted
maintenance closeout --project . --since HEAD~1
maintenance closeout --project . --change-manifest ./change.json
```

Minimal `change-manifest`:

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

## Pack fallback

Only when a subagent is unavailable:

```sh
maintenance closeout --project . --git uncommitted --pack --max-lines 200
```

`pack.md` contains candidate paths, tokens, hit lines, and a little context, bounded by `--max-lines`. It is not a long-term source of truth.

## Waivers and reporting

Extracted tokens carry a confidence level: tokens with usage evidence (env access patterns, `.env` assignments, compose `environment` keys, flags, config keys) are high confidence and create `missing`/`stale` obligations; bare uppercase words are listed as low-confidence reference only. When a high-confidence token is still noise, waive it instead of padding the docs:

```sh
maintenance waive --project . SOME_TOKEN --reason "generated constant, not user-facing"
```

Waivers land in `.doc-maintenance/waivers.toml` with the reason and date; `closeout` and `verify` skip waived tokens and record them as `ignored_tokens` in the manifest. Never add an information-free token mention just to make `verify` pass.

`verify` writes an `outcome.json` into the run directory. Summarize recent runs with:

```sh
maintenance report --project . --last 5
```

Each line shows the run id, change source, changed-file count, high/low token counts, waived and missing counts, and the verify result (`passed` / `failed` / `unverified`).

## Install into a skill package

For end users, the simplest path is to download the `doc-maintenance-skill-<platform>` bundle from Releases and unpack the `doc-maintenance/` folder into your agent's skills directory; the binary ships inside `bin/`. Adding it to `PATH` is optional, and automated installers should ask before modifying it. The steps below build that package from source.

```sh
cargo build --release
./scripts/copy-release.sh   # or .\scripts\copy-release.ps1 on Windows
```

The script copies only into `skill/doc-maintenance/bin/`. It never overwrites a global skills directory. Confirm the source and the release build match before installing globally.

## Verify the build

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```
