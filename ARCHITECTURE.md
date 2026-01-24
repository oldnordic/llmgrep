# Architecture

llmgrep is a read-only query tool over Magellan's SQLite graph database.

## Components

- CLI: parses args and dispatches search mode
- Query layer: reads graph_entities/graph_edges for symbols, references, calls
- Output layer: wraps results in schema-aligned JSON responses

## Data flow

1. CLI receives query and options
2. Query layer executes deterministic SQL
3. Results are ranked and normalized
4. Output layer emits JSON or human text

See `docs/ARCHITECTURE.md` for more detail.
