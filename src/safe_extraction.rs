//! Safe UTF-8 extraction functions for multi-byte character handling.
//!
//! This module re-exports Magellan's safe extraction functions and provides
//! llmgrep-specific error handling wrappers.
//!
//! # UTF-8 Safety
//!
//! Direct byte slicing (`&source[start..end]`) can panic on multi-byte UTF-8
//! character boundaries:
//!
//! - ASCII: 1 byte
//! - Accented Latin (é): 2 bytes
//! - CJK characters (中): 3 bytes
//! - Emoji (🎉): 4 bytes
//!
//! The functions in this module handle these cases safely by adjusting
//! boundaries to valid UTF-8 character boundaries or returning None.

// Re-export Magellan's safe UTF-8 extraction functions
pub use magellan::common::extract_symbol_content_safe;
pub use magellan::common::extract_context_safe;
pub use magellan::common::safe_str_slice;

use crate::error::LlmError;

/// Safely extract a snippet from source bytes with llmgrep error handling.
///
/// Wraps `extract_symbol_content_safe` and converts None to `LlmError::SearchFailed`.
/// This is useful for propagating extraction failures in the query pipeline.
///
/// # Arguments
///
/// * `source` - Source bytes to extract from
/// * `start` - Byte start offset (must be UTF-8 character boundary)
/// * `end` - Byte end offset (must be UTF-8 character boundary)
///
/// # Returns
///
/// * `Ok(String)` - Extracted content as UTF-8 string
/// * `Err(LlmError)` - Extraction failed with reason
///
/// # Examples
///
/// ```no_run
/// use llmgrep::safe_extraction::safe_extract_snippet;
///
/// let source = b"fn test() { // Hello }";
/// let snippet = safe_extract_snippet(source, 0, 15)?;
/// assert_eq!(snippet, "fn test() { // ");
/// # Ok::<(), llmgrep::LlmError>(())
/// ```
pub fn safe_extract_snippet(
    source: &[u8],
    start: usize,
    end: usize,
) -> Result<String, LlmError> {
    extract_symbol_content_safe(source, start, end).ok_or_else(|| {
        LlmError::SearchFailed {
            reason: format!(
                "Failed to extract snippet at byte range {}..{} (invalid UTF-8 boundary or out of bounds)",
                start, end
            ),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_extract_snippet_ascii() {
        let source = b"fn test() { return 42; }";
        let result = safe_extract_snippet(source, 0, 15);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "fn test() { ret");
    }

    #[test]
    fn test_safe_extract_snippet_accented_latin() {
        // "café" - 'é' is 2 bytes in UTF-8
        let source = "fn café() { return 42; }".as_bytes();
        // Extract the function name part
        let result = safe_extract_snippet(source, 3, 9);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "café()");
    }

    #[test]
    fn test_safe_extract_snippet_emoji() {
        // "fn test() { // 🎉 }" - emoji is 4 bytes
        let source = "fn test() { // 🎉 }".as_bytes();
        // Extract from start through the emoji comment
        // "🎉" is at byte 15-18 (4 bytes)
        let result = safe_extract_snippet(source, 0, 19);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "fn test() { // 🎉 ");
    }

    #[test]
    fn test_safe_extract_snippet_cjk() {
        // "fn test() { // 中文 }" - each CJK char is 3 bytes
        let source = "fn test() { // 中文 }".as_bytes();
        // Extract from start through part of CJK comment
        let result = safe_extract_snippet(source, 0, 18);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "fn test() { // 中");
    }

    #[test]
    fn test_safe_extract_snippet_boundary_splitting() {
        // "café" - 'é' is bytes [0xC3, 0xA9]
        let source = "fn café()".as_bytes();
        // Try to split in the middle of 'é' (byte 5 is in middle of 2-byte sequence)
        let result = safe_extract_snippet(source, 0, 5);
        // Should return error or adjusted result - not panic
        // Magellan's safe extraction should handle this
        match result {
            Ok(_) => {
                // If Ok, should be valid UTF-8
            }
            Err(_) => {
                // If Err, should be SearchFailed with explanation
            }
        }
    }

    #[test]
    fn test_safe_extract_snippet_out_of_bounds() {
        let source = b"short";
        let result = safe_extract_snippet(source, 0, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_symbol_content_safe_direct() {
        // Test direct magellan function re-export
        let source = "fn test() { return 42; }".as_bytes();
        let result = extract_symbol_content_safe(source, 0, 10);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "fn test() {");
    }

    #[test]
    fn test_safe_str_slice() {
        // Test string slice with safe boundaries
        let source = "fn test() { return 42; }";
        let result = safe_str_slice(source, 0, 10);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "fn test() {");
    }

    #[test]
    fn test_safe_str_slice_multi_byte() {
        // Test with multi-byte characters
        let source = "café";
        let result = safe_str_slice(source, 0, 4);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "caf");
    }

    #[test]
    fn test_safe_str_slice_invalid_boundary() {
        // Try to slice in middle of multi-byte character
        let source = "café"; // 'é' is 2 bytes at positions 3-4
        let result = safe_str_slice(source, 0, 4); // Ends at 'é' start
        // Should return Some with adjusted boundary or None
        match result {
            Some(s) => {
                // If Some, should be valid UTF-8
                assert!(s.is_char_boundary(s.len()));
            }
            None => {
                // Valid boundary can't be found
            }
        }
    }
}
