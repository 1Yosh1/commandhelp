# Handoff — CommandHelp (`chelp`)

_Last updated: 2026-10-04 · branch `master` @ `ca14671` (pushed, in sync with `origin/master`)_

## What this is

A Rust CLI that gives every terminal command **ghost-text autocomplete** (served by a
background daemon inside a **15 ms** budget) and turns **plain English into a
safety-rated command** via an interactive modal (`Ctrl+Space`). Spec lives in
`docs/superpowers/specs/2026-09-06-ai-cli-assistant-design.md`; task briefs in
`.superpowers/sdd/`.

## Architecture in one screen

```
shell hook (zsh/bash/fish/pwsh, src/hooks/)
  ├─ ghost text  → chelp complete -- <buffer>   → daemon (unix socket, 15ms budget)
  └─ Ctrl+Space  → chelp query <buffer> --out-file <tmp>
                        │
chelp query:  provider HTTP (ai.rs) → modal (tui.rs, ratatui + crossterm)
                        │  writes "run|edit\n<command>" to the out-file (empty = cancelled)
                        ▼
              hook executes (run) or lands it on the prompt (edit)
daemon: on-demand --help/man crawl (crawler.rs) → parse (parser.rs) → SQLite (storage.rs)
state:  everything under $CHELP_HOME (fallback ~/.chelp): config.toml, data.db, chelp.log
socket: /tmp/$CHELP_SOCKET (fallback chelp-ipc); stale sockets reclaimed after probe
```

## Non-negotiable conventions (do not regress these)

- **Out-file protocol**: hooks NEVER capture modal stdout. `--out-file` receives
  `run|edit\n<command>`; an empty file means cancelled. Diagnostics go to
  `~/.chelp/chelp.log` (1 MB rotating) + stderr, which hooks silence with `2>/dev/null`.
- **`crossterm` must keep the `use-dev-tty` feature** ([Cargo.toml](Cargo.toml)).
  On macOS a freshly opened `/dev/tty` fd cannot be kqueue-registered (EINVAL), which
  killed modal input whenever stdin was not a tty — i.e. inside every zsh ZLE widget.
  Guarded by the PTY test `modal_still_reads_keys_when_stdin_is_not_a_tty`
  (proven: removing the feature makes it fail with `Failed to initialize input reader`).
- **Destructive gate (spec §5.4)**: first `Enter` only arms; `y` executes;
  `Tab`/`e` always backs out to the prompt.
- **Bash < 4 is a deliberate no-op** (`BASH_VERSINFO` guard): macOS' stock bash 3.2
  runs `bind -x` but cannot read/write `READLINE_LINE`.
- **15 ms `COMPLETE_BUDGET`** (`src/ipc.rs`) — hooks must degrade to no-suggestion,
  never stall typing.
- Removed and should stay removed: `recipes`, `repo_indexer`, `dlp`, `site` marketing
  copy of old, `demo.tape`, `docs/launch_kit.md`, deps `nucleo-matcher`/`directories`.

## Verification (how to check your changes)

```bash
export PATH=$HOME/.cargo/bin:$PATH
cargo test            # 16 suites / 50 tests, must stay green
cargo clippy --all-targets   # currently 0 warnings
```

- PTY suite: `tests/pty_interactive_test.rs` (modal Enter/Esc, destructive gate,
  non-tty stdin, on-demand indexing) — includes a mock OpenAI server, fully hermetic.
- Hook contract + `zsh -n`/`bash -n`: `tests/shell_test.rs`.
- Parser corpus: 36 fixtures in `tests/fixtures/` (spec §8.1 wants ≥30, enforced).
- **`cargo fmt --check` currently FAILS** — pre-existing repo-wide drift plus mixed
  CRLF in `src/parser.rs`. Not auto-fixed to keep commit history readable; a dedicated
  `cargo fmt` commit is the intended cleanup.
- The external 18-check live shell harness (`verify_hooks.py`, incl. a source-built
  bash 5.2 at `/tmp/bash5`) was lost to a `/tmp` purge. Rebuilding it is open work.

## Open work (priority order)

1. **`? <query>` prefix trigger (spec §3.2)** — natural language should also activate
   by typing `? <task>` + Enter. Only `Ctrl+Space` exists today. Needs a cross-shell
   Enter-binding design; bash `bind -x` on Enter is the hard case (handler cannot
   conditionally re-dispatch the key).
2. **Durable live-shell harness** — port the 18-check zsh/bash suite into the repo
   with a bash 5 bootstrap so it survives `/tmp` wipes.
3. **Live fish + PowerShell verification** — both have contract tests only; nothing
   has been run on Windows.
4. **`cargo fmt` pass** (own commit) and decide on CRLF normalization.
5. **MIT `LICENSE` file** — README claims MIT but no file exists (page links to the
   README section instead).
6. **Deploy the site** — `site/index.html` + `vercel.json` root rewrite are ready;
   needs the Vercel account/CLI. `.freebuff/` is intentionally untracked.
7. Cosmetic spec deviations, consciously accepted: modal is an alt-screen takeover
   (spec drew it "below the prompt"); ghost text uses terminal styling, not forced
   ANSI-8 dim gray.

## Where things live

| Area | Files |
|---|---|
| CLI entry, out-file protocol | `src/main.rs` |
| Modal + key handling + gate | `src/tui.rs` |
| Completion pipeline, budget | `src/complete.rs`, `src/ipc.rs` |
| Crawl/parse/store | `src/crawler.rs`, `src/parser.rs`, `src/storage.rs` |
| Shell hooks (source of truth) | `src/hooks/*` (embedded via `src/shell.rs`) |
| State paths, logging | `src/config.rs`, `src/log.rs` |
| Product page | `site/index.html`, `vercel.json` |
