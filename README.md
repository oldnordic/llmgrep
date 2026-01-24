# llmgrep

llmgrep is a smart grep CLI built to be used with Magellan (code map + SQLite) and emits schema-aligned JSON for LLM workflows.

Magellan repository: https://github.com/oldnordic/magellan

Compatibility:
- Magellan schema version: 1.0.0 (JsonResponse wrapper)
- Magellan export version: 2.0.0
- Minimum Magellan version: 1.7.0

## What it does

- Searches symbols, references, and calls from Magellan's database
- Emits deterministic, schema-aligned JSON output
- Supports regex, ranking, stable IDs, and optional context/snippets

## Install

```
cargo build --release
cp target/release/llmgrep /home/feanor/.local/bin/llmgrep
```

## Quick start

1) Build a database with Magellan:

```
magellan watch --root /path/to/repo --db /tmp/repo.db
```

2) Query it with llmgrep:

```
llmgrep search --db /tmp/repo.db --query "parse" --output json
```

## Docs

- `docs/USAGE.md`
- `docs/CLI_PATTERNS.md`
- `docs/JSON_EXPORT_FORMAT.md`
- `docs/SCHEMA_REFERENCE.md`

## Tested scope

Verified locally on this repo:
- Magellan indexing to `/tmp/llmgrep.db`
- `llmgrep search` in `symbols`, `references`, `calls`, and `auto` modes
- JSON output with context/snippet/score flags

## License

GPL-3.0-only. See `LICENSE.md`.
