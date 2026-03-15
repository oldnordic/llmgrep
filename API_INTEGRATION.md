# Magellan API Integration

This document describes how llmgrep integrates with Magellan's APIs and contracts.

## Version Requirements

- **Magellan:** 3.0.0+ (Geometric backend support)
- **sqlitegraph:** 2.0.7+

## Backend Opening

llmgrep uses `Backend::detect_and_open()` to automatically detect and open the appropriate backend:

```rust
pub fn detect_and_open(db_path: &Path) -> Result<Self, LlmError> {
    // 1. Check file extension for .geo
    if extension == "geo" {
        return GeometricBackend::open(db_path);
    }

    // 2. Read header bytes for other formats
    let header = read_first_16_bytes(db_path);

    // 3. Check for V3 format magic: "SQLTGF"
    if header == "SQLTGF" {
        return NativeV3Backend::open(db_path); // requires feature
    }

    // 4. Check for SQLite format: "SQLite format 3\0"
    if header == "SQLite format 3\0" {
        return SqliteBackend::open(db_path);
    }
}
```

## Geometric Backend Contract

### Opening

```rust
let backend = GeometricBackend::open(&Path::new("code.geo"))?;
// Returns: Result<GeometricBackend, LlmError>
```

### Path Normalization

All paths are normalized before queries to Magellan:

```rust
use magellan::graph::path_utils::normalize_path;

let normalized = normalize_path("./src/lib.rs", None)?;
// Result: "src/lib.rs" or absolute path if file exists
```

### Symbol Lookup Results

```rust
pub enum SymbolLookupResult {
    Unique(SymbolInfo),
    Ambiguous { path: String, name: String, candidates: Vec<SymbolInfo> },
    NotFound,
}
```

### Chunk Retrieval

```rust
let chunks = backend.get_code_chunks_for_symbol(path, symbol_name)?;
// Returns: Result<Vec<CodeChunk>, Error>

pub struct CodeChunk {
    pub id: Option<i64>,
    pub file_path: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub content: String,
    pub symbol_name: Option<String>,
    pub symbol_kind: Option<String>,
}
```

### Symbol Queries

```rust
// Find symbols by name
let symbols = backend.find_symbols_by_name_info("parse");

// Get all symbols
let all = backend.get_all_symbols()?;

// Get symbols in file
let file_symbols = backend.symbols_in_file("src/lib.rs")?;

// Find by exact FQN
let symbol = backend.find_symbol_by_fqn_info("my_crate::module::function");

// Get callers/callees
let callers = backend.get_callers(symbol_id);
let callees = backend.get_callees(symbol_id);
```

## Ambiguity Handling

llmgrep propagates Magellan's ambiguity errors explicitly:

```rust
use magellan::graph::path_utils::AmbiguityError;

match backend.find_symbol_id_by_path_and_name(path, name) {
    Ok(Some(id)) => // Unique match found
    Ok(None) => // Not found
    Err(AmbiguityError { candidates }) => {
        // Multiple symbols match - return structured error
        return Err(LlmError::AmbiguousSymbolName {
            name: name.to_string(),
            count: candidates.len(),
        });
    }
}
```

## Error Mapping

| Magellan Error | llmgrep Error | Code |
|----------------|---------------|------|
| `BackendOpenFailed` | `BackendDetectionFailed` | LLM-E110 |
| `AmbiguityError` | `AmbiguousSymbolName` | LLM-E106 |
| Symbol not found | `SymbolNotFound` | LLM-E112 |
| Chunk not available | `ChunksNotAvailable` | LLM-E113 |

## Delegated Operations

llmgrep delegates the following to Magellan (does not implement):

- **Indexing** — Use `magellan watch`
- **Re-opening** — Handled by Magellan's backend
- **Persistence** — Handled by Magellan's storage layer
- **Schema migrations** — Handled by Magellan versioning

## Testing

Integration tests verify the contract:

```bash
cargo test --test magellan_geo_contract
cargo test --test component_integration_geometric
cargo test --test unit_integration_geometric
```
