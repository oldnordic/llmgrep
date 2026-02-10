# External Integrations

**Analysis Date:** 2026-02-10

## APIs & External Services

**Magellan (Code Graph Indexer):**
- External tool providing code graph database
- SDK/Client: magellan 2.2.1 crate dependency
- Auth: None (local file-based database)
- Integration pattern: Read-only client for Magellan databases

**Shell Commands (Magellan CLI):**
- Executes `magellan` subprocess for algorithm operations
- Commands used: `magellan find`, `magellan reachable`, `magellan dead-code`, `magellan cycles`, `magellan condense`, `magellan paths`
- Location: `src/algorithm.rs`

## Data Storage

**Databases:**
- **SQLite Backend:**
  - Connection: Direct file access via rusqlite
  - Client: rusqlite 0.31
  - Implementation: `src/backend/sqlite.rs`

- **Native-V2 Backend:**
  - Connection: magellan::CodeGraph API
  - Format: GraphFile (sqlitegraph custom format)
  - Implementation: `src/backend/native_v2.rs`
  - Requires: `--features native-v2` compile flag

**Database Schema Dependencies:**
- `graph_entities` table - Symbol definitions
- `graph_edges` table - References and call relationships
- `ast_nodes` table - Abstract syntax tree data
- `code_chunks` table - Pre-extracted code snippets
- `symbol_metrics` table - Complexity and fan-in/out metrics
- `kv_entries` table (native-v2) - Key-value store for FQN and label lookups

**File Storage:**
- Local filesystem only - No remote storage
- Database files: Typically `.codemcp/codegraph.db`

**Caching:**
- None - Direct database queries only

## Authentication & Identity

**Auth Provider:**
- None (local tool, no network authentication)

## Monitoring & Observability

**Error Tracking:**
- Custom error codes (LLM-E001 through LLM-E999)
- Error types defined in `src/error.rs`
- Structured error messages for LLM consumption

**Logs:**
- No logging framework (CLI tool, stdout/stderr only)
- `--show-metrics` flag for performance timing
- Three-phase metrics: backend detection, query execution, output formatting

## CI/CD & Deployment

**Hosting:**
- crates.io - Published as `llmgrep` crate
- GitHub - Source repository: https://github.com/oldnordic/llmgrep

**CI Pipeline:**
- Not detected in repository (GitHub Actions or similar not present)

**Distribution:**
- `cargo install llmgrep` - Primary installation method
- Pre-built binaries: Not detected (source-only distribution)

## Environment Configuration

**Required env vars:**
- None (CLI-driven configuration)

**Secrets location:**
- Not applicable (no authentication/secrets required)

**Configuration sources:**
- Command-line arguments (clap-derived)
- Feature flags at compile time
- Database file path (typically `.codemcp/codegraph.db`)

## Webhooks & Callbacks

**Incoming:**
- None (CLI tool, no server)

**Outgoing:**
- Shell commands to Magellan CLI (subprocess execution)
- No HTTP/webhook calls

## Inter-Tool Communication

**Magellan Integration:**
- Reads databases created by `magellan watch`
- Shell-out pattern for graph algorithms
- Shared database format via sqlitegraph

**Related Tools (ecosystem):**
- **Mirage** - CFG analysis tool (separate binary)
- **splice** - Code editing tool (separate binary)
- **sqlitegraph** - Shared database foundation

**Coordination:**
- All tools share same database file path (`.codemcp/codegraph.db`)
- No IPC - file-based coordination only

## File Format Dependencies

**Input Formats:**
- Magellan SQLite database (legacy)
- Magellan GraphFile format (native-v2)
- SymbolSet JSON files (for algorithm-based filtering)

**Output Formats:**
- Human-readable text (default)
- JSON (schema-aligned for LLM consumption)
- Pretty-printed JSON

---

*Integration audit: 2026-02-10*
