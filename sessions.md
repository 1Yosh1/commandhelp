# Sessions

Work log for the audit → remediation → ship arc. All sessions on branch `master`,
repo `1Yosh1/commandhelp`. Times are local.

---

## Session 1 — 2026-10-03 · Four-dimension audit

**Request:** harsh audit of the codebase across SPEC / DESIGN / CORRECTNESS / QUALITY.

**Outcome — scores:**

| Dimension | Score |
|---|---|
| SPEC | 3 |
| DESIGN | 5 |
| CORRECTNESS | 4 |
| QUALITY | 5 |

Headline findings: spec gaps (fixture corpus <30, no enforced 15 ms budget,
destructive gate missing), dead/flagged extras (recipes, repo-indexer, DLP, static
site, launch kit, demo tape), state scattered outside `~/.chelp`, hooks capturing
modal ANSI into prompts, tests that passed for the wrong reasons (false positives).

---

## Session 2 — 2026-10-03/04 · "fix all of these"

Scope decision: user chose **"Remove all flagged extras"**.

**Deleted:** `src/dlp.rs`, `src/recipes.rs`, `src/repo_indexer.rs`, their three test
files, `site/index.html` (old marketing page), `docs/launch_kit.md`, `demo.tape`;
deps `nucleo-matcher`, `directories`; the vercel `/` → site rewrite.

**Rewritten/created:** `config.rs` (`$CHELP_HOME` single owner), `log.rs` (1 MB
rotating `chelp.log`), `complete.rs` (15 ms budget, background crawl + placeholder),
`crawler.rs` (1500 ms probes, mtime tracking), `ipc.rs` (transport-only, stale-socket
reclaim), `tui.rs` (destructive Enter→`y` gate, non-tty fallback), `main.rs`
(`run|edit\ncommand` out-file protocol), `parser.rs` (bracketed switches, OnceLock),
all four hooks under `src/hooks/`, plus `auth.rs`, `safety.rs`, `ai.rs`.

**Tests:** 36 parser fixtures + `parser_fixtures_test.rs`; PTY suite rewritten with a
mock OpenAI server and sandboxed `CHELP_HOME`; 4-shell hook contract tests; hermetic
auth/ai tests.

**Verification:** `cargo test` green (16 suites).

---

## Session 3 — 2026-10-04 · The zsh modal bug (root cause hunt)

**Symptom:** in zsh, `Ctrl+Space` opened the modal but `chelp` exited instantly with
nothing written to the out-file — the hook read it as *cancel*.

**Journey:**
1. Instrumented the hook (captured stderr/exit code) → `Error: I/O error: Failed to
   initialize input reader`, exit 1. (First attempt accidentally redirected stdout,
   which itself forced the non-tty Edit fallback — red herring.)
2. Reproduced outside zsh: PTY with stdin `/dev/null` → same failure.
3. Bisected crossterm's reader init with a scratch crate → **mio `register` of a
   freshly opened `/dev/tty` fd returns EINVAL on macOS** (kqueue quirk). Inherited
   fd 0 registers fine; `/dev/tty` never does. Inside a zsh ZLE widget stdin is not a
   tty, so crossterm always takes the `/dev/tty` path → dead input.

**Fix:** one line — `crossterm = { version = "0.27", features = ["use-dev-tty"] }`
(polls with `poll(2)` instead of kqueue).

**Verification:** stdin-not-a-tty repro passes; real zsh widget: modal renders, Enter
executes; `/tmp/verify_hooks.py` **18/18** (fixed two bad assertions: ghost-text
window marked after typing; bash `ls` assertion expected a literal path `ls` never
prints); `cargo test` green; README rewritten for the new hook behavior; scratch
probes cleaned.

---

## Session 4 — 2026-10-04 · Regression guard, commits, re-audit

- Added `modal_still_reads_keys_when_stdin_is_not_a_tty` (PTY + `/bin/sh -c 'exec … <
  /dev/null'`), refactored the harness to `run_program_in_pty`, made `Step::Send`
  tolerate writes to a dead pty.
- **Mutation-checked the guard:** dropping `use-dev-tty` makes the test fail with the
  original error (exit 101); restoring it passes.
- **Re-audit scores: SPEC 3→6, DESIGN 5, CORRECTNESS 4→5, QUALITY 5.** Remaining
  SPEC gap: `? <query>` prefix trigger (§3.2) never built.
- Fixed the 3 clippy warnings the re-audit surfaced (derivable `Default`, regex
  compiled inside the flag-parse loop, `map_or` → `is_some_and`).
- **Commits:** `853ff33` remediation (73 files) · `b3ba9b6` README · `f0da52e` lint.
- Known failing check, deliberately left: `cargo fmt --check` (repo-wide pre-existing
  drift + mixed CRLF in `parser.rs`).

---

## Session 5 — 2026-10-04 · Product page + publish

**Request:** "make a products page for this, clean and functional … deployable to
Vercel, link my git and installation docs, take inspo from the Homebrew page."

- Built [site/index.html](site/index.html) — single-file, brew.sh-style layout:
  quick-install hero, features, workflow, per-platform install cards with copy
  buttons, documentation cards linking the repo. Animated terminal demo.
- Added the `/` → `/site/index.html` rewrite to `vercel.json`.
- Browser-verified at 439 px and 1280 px: fixed two real bugs found by testing —
  grid tracks blown out by the unbreakable curl one-liner (`min-width: 0`) and copy
  feedback stalling behind `clipboard.writeText` (now optimistic). Zero console
  errors; anchors matched against GitHub's own emitted hrefs on the live page.
- **Pushed everything:** `ca14671 feat(site): add product page…` plus the three
  earlier commits (`461e47f..ca14671`). First push failed on a transient network
  outage; retry exit 0, branch in sync.

**Current state:** `master` = `origin/master` = `ca14671`, tree clean except
untracked `.freebuff/`. Site is deploy-ready but not deployed. See
[handoff.md](handoff.md) for open work.
