# llmgrep Manual

llmgrep is a read-only query tool for Magellan's code map. It does not build or modify the database; Magellan owns indexing and freshness.

Tested locally: search modes (symbols/references/calls/auto) with JSON output, context, snippets, and scores on a Magellan DB built from this repo.

## Core command

```
llmgrep search --db <FILE> --query <STRING> [--mode symbols|references|calls|auto] [--output human|json|pretty]
```

Key options:
- `--db`: Required. Path to Magellan SQLite database.
- `--mode`: Search mode (symbols, references, calls, auto).
- `--regex`: Treat query as regex.
- `--limit`: Max results per mode (auto mode uses `--auto-limit`).
- `--with-context` / `--context-lines` / `--max-context-lines`: Add context in JSON.
- `--with-snippet` / `--max-snippet-bytes`: Add snippet in JSON.
- `--with-fqn`: Include FQN fields in JSON.
- `--fields`: JSON-only field selector (overrides `--with-*`).

See `docs/USAGE.md` for full usage examples.
