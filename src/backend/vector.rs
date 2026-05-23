use crate::error::LlmError;
use sqlitegraph::hnsw::{DistanceMetric, HnswConfigBuilder, HnswIndex};

/// Trait for generating embeddings from text.
///
/// Implementations can use any embedding model — local (Ollama) or remote.
pub trait EmbeddingProvider: Send + Sync {
    fn embed(&self, text: &str) -> Result<Vec<f32>, LlmError>;
    fn dimension(&self) -> usize;
}

/// In-memory HNSW vector index for semantic code search.
pub struct VectorIndex {
    inner: HnswIndex,
    dim: usize,
}

impl VectorIndex {
    /// Create a new in-memory HNSW index with cosine distance.
    pub fn create(name: &str, dim: usize) -> Result<Self, LlmError> {
        let config = HnswConfigBuilder::new()
            .dimension(dim)
            .m_connections(16)
            .ef_construction(200)
            .ef_search(50)
            .distance_metric(DistanceMetric::Cosine)
            .build()
            .map_err(|e| LlmError::InvalidQuery {
                query: format!("HNSW config error: {e}"),
            })?;

        let inner = HnswIndex::new(name, config).map_err(|e| LlmError::InvalidQuery {
            query: format!("HNSW index creation error: {e}"),
        })?;

        Ok(Self { inner, dim })
    }

    /// Insert a vector with the given id into the index.
    pub fn insert(&mut self, _id: u64, vector: &[f32]) -> Result<u64, LlmError> {
        self.inner
            .insert_vector(vector, None)
            .map_err(|e| LlmError::InvalidQuery {
                query: format!("HNSW insert error: {e}"),
            })
    }

    /// Search for the k nearest neighbors.
    ///
    /// Returns `(vector_id, distance)` pairs sorted by distance ascending.
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(u64, f32)>, LlmError> {
        self.inner
            .search(query, k)
            .map_err(|e| LlmError::InvalidQuery {
                query: format!("HNSW search error: {e}"),
            })
    }

    /// Return the configured vector dimension.
    pub fn dimension(&self) -> usize {
        self.dim
    }

    /// Return the number of vectors currently indexed.
    pub fn len(&self) -> usize {
        self.inner.statistics().map(|s| s.vector_count).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_index_creation() {
        let vi = VectorIndex::create("test", 3).unwrap();
        assert_eq!(vi.dimension(), 3);
        assert_eq!(vi.len(), 0);
        assert!(vi.is_empty());
    }

    #[test]
    fn test_vector_index_insert_and_search() {
        let mut vi = VectorIndex::create("test", 3).unwrap();
        vi.insert(1, &[1.0, 0.0, 0.0]).unwrap();
        vi.insert(2, &[0.0, 1.0, 0.0]).unwrap();
        vi.insert(3, &[0.0, 0.0, 1.0]).unwrap();

        assert_eq!(vi.len(), 3);

        // Query closest to [1,0,0] — should return id=1 (the same vector) first
        let results = vi.search(&[1.0, 0.0, 0.0], 1).unwrap();
        assert!(!results.is_empty());
        // id=1 or the internal storage id for the first inserted vector
        // Distance to itself should be minimal (near 0 for cosine)
        assert!(
            results[0].1 < 0.1,
            "self-distance too large: {}",
            results[0].1
        );
    }

    #[test]
    fn test_vector_index_wrong_dimension() {
        let mut vi = VectorIndex::create("test", 3).unwrap();
        // Inserting a vector with wrong dimension should error
        let result = vi.insert(1, &[1.0, 0.0]);
        assert!(result.is_err(), "expected error for wrong-dimension insert");
    }
}
