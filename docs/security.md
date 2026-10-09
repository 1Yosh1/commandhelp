# CommandHelp (chelp) — Security & Data Flow

_Auditor-facing summary of every byte chelp reads, writes, stores and sends.
Last updated: 2026-10-09._

## 1. Data flow in one screen

```
your keystrokes (shell input)
        ├── ghost text path: chelp complete -- <buffer>
        │        └── over the daemon socket: the current buffer only
        └── query path: chelp query <buffer>
                 ├── to your AI provider (Gemini/OpenAI/Anthropic/Ollama/…):
                 │       the current buffer ONLY — one HTTP request, nothing else
                 └── nowhere else, ever
```

What **never** happens:

- No command history is written to disk by chelp, ever.
- No telemetry, metrics, update checks, or outbound pings exist in the binary.
- Nothing is sent to chelp's makers or any third party besides your own
  configured AI provider.
- Ollama mode is 100% offline: the only open sockets are your local Ollama
  endpoint and the IPC socket.

## 2. The daemon and its socket

- One background daemon serves completion requests within a 15 ms budget.
- Transport: a single Unix-domain (or named, on Windows) local socket.
  - Name defaults to `chelp-ipc`; nothing binds on TCP.
  - **Opt-out kills the daemon entirely**: set `CHELP_DISABLE_DAEMON=1` and the
    hooks, the auto-spawn path and the lazy spawn path are all silent no-ops.
    Completions degrade to none; `chelp query` still talks to your provider
    directly, so you can keep the modal without the daemon. `chelp privacy`
    prints the effective state.
- The socket carries framed JSON containing only the current input buffer
  request/response. Closing/audit story: a crashed or killed daemon leaves a
  stale socket file that is reclaimed after a probe on next use.

## 3. What SQLite stores (`~/.chelp/data.db`)

- Parsed `--help`/man-page **flag schemas only** — tool names, flags, help text.
- It does not contain: your typed commands, buffers sent for completion, query
  results, provider responses, or any history.
- Purge at any time:

```bash
chelp cache clear    # deletes everything in the schema cache and VACUUMs
chelp cache info     # shows the path and row count for verification
```

## 4. Credentials

- Recommended: keep the API key out of `~/.chelp/config.toml` entirely and
  inject it from your secret manager:

```bash
# 1Password
export CHELP_API_KEY="$(op read 'op://Vault/chelp/api-key')"
# Bitwarden
export CHELP_API_KEY="$(bw get password chelp-api-key)"
# HashiCorp Vault
export CHELP_API_KEY="$(vault kv get -field api-key secret/chelp)"
```

  `CHELP_API_KEY` overrides whatever is in the config file.
- For enterprise: keys never leave the machine except inside the request to
  your chosen provider endpoint.

## 5. The crawl path (spawning other binaries)

The on-demand indexer runs `<binary> --help` / `man <binary>` and captures
stdout:

- Arguments are fixed strings; no shell interpolation, no user buffer is ever
  included in what is executed.
- Inherited environment is not modified; the crawl does not open network
  connections by itself. A timeout bounds every crawl.

Recommended workstation profiles (verified pattern): run the daemon with
`systemd-run --user -p PrivateNetwork=yes …` (Linux) or sandbox-exec
(macOS) if your compliance regime requires network isolation of the daemon;
`CHELP_DISABLE_DAEMON=1` is the simplest auditable profile of all.

## 6. Verifying the claims yourself

```bash
strings $(which chelp) | grep -iE "telemetry|analytics|sentry"   # no hits
chelp privacy          # prints this audit in the same order as this doc
chelp cache info       # see exactly how much is stored on your machine
chelp --dump-ast docker  # replay the parser on any binary, offline
```

To confirm zero outbound traffic to non-provider hosts, run chelp behind any
outbound observer (e.g. Little Snitch, `lsof -i`, eBPF): you should see
connections only to your configured provider endpoint.
