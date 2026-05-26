use llmgrep::error::LlmError;

pub fn run_vector_create(name: &str, dim: usize) -> Result<(), LlmError> {
    use llmgrep::backend::vector::VectorIndex;
    let vi = VectorIndex::create(name, dim)?;
    println!(
        "{{\"status\":\"ok\",\"index\":{:?},\"dim\":{},\"vectors\":{}}}",
        name,
        vi.dimension(),
        vi.len()
    );
    Ok(())
}

pub fn run_vector_search(query: &str, index: &str, limit: usize) -> Result<(), LlmError> {
    use llmgrep::backend::vector::VectorIndex;
    let vector: Result<Vec<f32>, _> = query.split(',').map(|s| s.trim().parse::<f32>()).collect();
    let vector = vector.map_err(|_| LlmError::InvalidQuery {
        query: format!(
            "Failed to parse query vector: expected comma-separated floats, got: {query}"
        ),
    })?;
    let dim = vector.len();
    let vi = VectorIndex::create(index, dim)?;
    let results = vi.search(&vector, limit)?;
    let json_results: Vec<serde_json::Value> = results
        .iter()
        .map(|(id, dist)| serde_json::json!({"id": id, "distance": dist}))
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "index": index,
            "query_dim": dim,
            "results": json_results,
        }))
        .unwrap_or_default()
    );
    Ok(())
}
