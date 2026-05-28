# Architecture

llmgrep is a read-only query tool for Magellan code graphs with multi-backend support.

## Components

```
llmgrep/
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Library exports
│   ├── cli.rs               # CLI argument parsing
│   ├── dispatch.rs          # Command dispatch routing
│   ├── commands/            # Command implementations (one file per command)
│   │   ├── search.rs        #   Symbol/reference/call/docs/facts search
│   │   ├── explore.rs       #   Intent-based search
│   │   ├── navigate.rs      #   Stepable graph traversal (CodeGraph::navigator)
│   │   ├── stats.rs         #   Code health statistics
│   │   ├── evolve.rs        #   Symbol evolution scoring
│   │   ├── ast.rs           #   AST tree query
│   │   ├── find_ast.rs      #   AST node search
│   │   ├── complete.rs      #   FQN autocomplete
│   │   ├── lookup.rs        #   Exact FQN lookup
│   │   ├── watch.rs         #   Watch mode
│   │   └── vector.rs        #   Vector search
│   ├── query/               # Query building and execution
│   │   ├── mod.rs           #   Search options
│   │   ├── builder.rs       #   SQL query construction
│   │   ├── symbols.rs       #   Symbol search queries
│   │   ├── references.rs    #   Reference search queries
│   │   ├── calls.rs         #   Call search queries
│   │   ├── explore.rs       #   Intent-based search logic
│   │   ├── stats.rs         #   Statistics computation
│   │   ├── evolve.rs        #   Evolution scoring
│   │   ├── telemetry.rs     #   Opt-in local telemetry
│   │   └── tests/           #   10 test files for query logic
│   ├── backend/
│   │   ├── mod.rs           # Backend trait and dispatcher
│   │   ├── geometric.rs     # Geometric (.geo) backend
│   │   ├── magellan_adapter.rs  # Contract-aware integration
│   │   ├── sqlite.rs        # SQLite backend
│   │   ├── schema_check.rs  # Schema version detection
│   │   └── vector.rs        # Vector search backend
│   ├── algorithm/
│   │   └── mod.rs           # Magellan algorithm integration
│   ├── ast/
│   │   ├── mod.rs           # AST filtering
│   │   └── language.rs      # Language-aware kind expansion
│   ├── output.rs            # Response formatting (JSON, human)
│   └── error.rs             # Error types with codes
```

## Backend Layer

The `Backend` enum provides runtime backend detection and dispatch:

```rust
pub enum Backend {
    Sqlite(SqliteBackend),
    Geometric(GeometricBackend),
}

impl Backend {
    pub fn detect_and_open(db_path: &Path) -> Result<Self, LlmError> {
        // 1. Check .geo extension
        // 2. Check SQLite header bytes
        // 3. Open appropriate backend
    }
}
```

### Backend Trait

All backends implement `BackendTrait`:

```rust
pub trait BackendTrait {
    fn search_symbols(&self, options: SearchOptions)
        -> Result<(SearchResponse, bool, bool), LlmError>;
    fn search_references(&self, options: SearchOptions)
        -> Result<(ReferenceSearchResponse, bool), LlmError>;
    fn search_calls(&self, options: SearchOptions)
        -> Result<(CallSearchResponse, bool), LlmError>;
    fn ast(&self, file: &Path, position: Option<usize>, limit: usize)
        -> Result<serde_json::Value, LlmError>;
    fn find_ast(&self, kind: &str)
        -> Result<serde_json::Value, LlmError>;
    fn complete(&self, prefix: &str, limit: usize)
        -> Result<Vec<String>, LlmError>;
    fn lookup(&self, fqn: &str, db_path: &str)
        -> Result<SymbolMatch, LlmError>;
    fn get_chunks_for_symbol(&self, file_path: &str, symbol_name: &str)
        -> Result<Vec<CodeChunk>, LlmError>;
}
```

## Query Flow

```
CLI Command
    │
    ▼
Backend::detect_and_open()
    │
    ▼
Backend::search_symbols(options)
    │
    ├──► GeometricBackend
    │       │
    │       ├──► normalize_path()
    │       │
    │       ├──► backend.find_symbols_by_name_info()
    │       │
    │       └──► filter by path/kind/language
    │
    ├──► SqliteBackend
    │       │
    │       └──► SQL query with JOINs
    │
    └──► GeometricBackend
            │
            ├──► normalize_path()
            │
            ├──► backend.find_symbols_by_name_info()
            │
            └──► filter by path/kind/language
```

## Magellan Adapter Layer

The `magellan_adapter` module provides contract-aware integration:

- **Path normalization** before all queries
- **Ambiguity handling** with explicit error types
- **Chunk retrieval** with graceful degradation
- **Path filtering** with normalized comparison

```rust
pub fn normalize_path_for_query(path: &str) -> String {
    match normalize_path(path, None) {
        Ok(normalized) => normalized,
        Err(_) => path.to_string(), // fallback
    }
}

pub fn paths_equivalent(path1: &str, path2: &str) -> bool {
    normalize_path_for_query(path1) == normalize_path_for_query(path2)
}
```

## Output Layer

All backends return structured response types:

```rust
pub struct SearchResponse {
    pub query: String,
    pub total_count: u64,
    pub results: Vec<SymbolMatch>,
    pub path_filter: Option<String>,
    pub kind_filter: Option<String>,
    pub notice: Option<String>,
}

pub struct SymbolMatch {
    pub match_id: String,
    pub span: Span,
    pub name: String,
    pub kind: String,
    pub fqn: Option<String>,
    pub symbol_id: Option<String>,
    pub complexity_score: Option<u64>,
    pub fan_in: Option<u64>,
    pub fan_out: Option<u64>,
    // ... additional fields
}
```

## Error Handling

All errors include:
- Error code (LLM-E###)
- User-friendly message
- Remediation hints

```rust
pub enum LlmError {
    #[error("LLM-E113: Code chunks not available for {backend} backend.\n\n{message}")]
    ChunksNotAvailable { backend: String, message: String },

    #[error("LLM-E106: Ambiguous symbol name '{name}': {count} matches.")]
    AmbiguousSymbolName { name: String, count: usize },
    // ...
}
```

## Integration Points

### Magellan GeometricBackend

llmgrep uses these APIs from Magellan:

```rust
// Opening
let backend = GeometricBackend::open(&geo_path)?;

// Queries
let symbols = backend.find_symbols_by_name_info(name);
let all = backend.get_all_symbols()?;
let by_file = backend.symbols_in_file(path)?;

// References
let callers = backend.get_callers(symbol_id);
let (callers, callees) = backend.get_references_bidirectional(symbol_id)?;

// Chunks
let chunks = backend.get_code_chunks_for_symbol(path, name)?;
```

### sqlitegraph

llmgrep uses sqlitegraph for:

- SQL query construction helpers
- Graph entity/edge schema constants
- Algorithm execution (when Mag CLI unavailable)

## Navigate Command

The `navigate` command differs from other commands: instead of constructing raw SQL queries, it uses magellan's `CodeGraph::navigator()` API for graph traversal. This provides depth-aware BFS over the call graph with structured node/edge results.

```
navigate --symbol X --callers --depth 2
    │
    ▼
CodeGraph::navigator()
    │
    ├──► resolve(name) → entity_id
    ├──► callers(entity_id, depth=2) → Vec<(depth, Node)>
    ├──► callees(entity_id, depth=2) → Vec<(depth, Node)>
    └──► edges(entity_id) → Vec<Edge>
```

## Data Flow Summary

1. **CLI** receives command → parses to `SearchOptions`
2. **Backend** detects format → opens appropriate backend
3. **Query** executes → filters by path/kind/language
4. **Adapter** normalizes paths → handles ambiguity
5. **Output** formats as JSON or human text
6. **CLI** prints result or exits with error code
