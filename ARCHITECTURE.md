# Architecture

llmgrep is a read-only query tool for Magellan code graphs with multi-backend support.

## Components

```
llmgrep/
├── src/
│   ├── main.rs              # CLI entry point, command parsing
│   ├── lib.rs               # Library exports
│   ├── backend/
│   │   ├── mod.rs           # Backend trait and dispatcher
│   │   ├── geometric.rs     # Geometric (.geo) backend implementation
│   │   ├── magellan_adapter.rs  # Contract-aware integration layer
│   │   ├── sqlite.rs        # SQLite backend implementation
│   │   └── native_v3.rs     # Native-V3 backend (reserved, disabled)
│   ├── query/
│   │   └── mod.rs           # Search options and query building
│   ├── output/
│   │   ├── mod.rs           # Response types (JSON, human)
│   │   └── json.rs          # JSON serialization
│   ├── algorithm/
│   │   └── mod.rs           # Magellan algorithm integration
│   ├── ast/
│   │   └── mod.rs           # AST filtering utilities
│   └── error.rs             # Error types with codes
```

## Backend Layer

The `Backend` enum provides runtime backend detection and dispatch:

```rust
pub enum Backend {
    Sqlite(SqliteBackend),
    #[cfg(feature = "native-v3")]
    NativeV3(NativeV3Backend),
    Geometric(GeometricBackend),
}

impl Backend {
    pub fn detect_and_open(db_path: &Path) -> Result<Self, LlmError> {
        // 1. Check .geo extension
        // 2. Read header for magic bytes
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
    └──► NativeV3Backend (reserved)
            │
            └──► CodeGraph API calls
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

## Data Flow Summary

1. **CLI** receives command → parses to `SearchOptions`
2. **Backend** detects format → opens appropriate backend
3. **Query** executes → filters by path/kind/language
4. **Adapter** normalizes paths → handles ambiguity
5. **Output** formats as JSON or human text
6. **CLI** prints result or exits with error code
