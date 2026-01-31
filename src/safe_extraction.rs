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
pub use magellan::common::extract_context_safe;
pub use magellan::common::extract_symbol_content_safe;
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
/// use llmgrep::error::LlmError;
///
/// let source = b"fn test() { // Hello }";
/// let snippet = safe_extract_snippet(source, 0, 15)?;
/// assert_eq!(snippet, "fn test() { // ");
/// # Ok::<(), LlmError>(())
/// ```
pub fn safe_extract_snippet(source: &[u8], start: usize, end: usize) -> Result<String, LlmError> {
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
        // "café" - 'é' is 2 bytes in UTF-8 (0xC3, 0xA9)
        let source = "fn café() { return 42; }";
        let bytes = source.as_bytes();
        // Extract the function name part
        // "caf" is bytes 3-6, 'é' is bytes 6-7, '(' is byte 8
        // To get "café", we need bytes 3-8 (end at '(')
        let result = safe_extract_snippet(bytes, 3, 8);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "café");
    }

    #[test]
    fn test_safe_extract_snippet_emoji() {
        // "fn test() { // 🎉 }" - emoji is 4 bytes
        let source = "fn test() { // 🎉 }";
        let bytes = source.as_bytes();
        // Extract from start through the emoji comment
        // "🎉" starts at byte 15, is 4 bytes (15-18)
        // If we pass 19, it gets adjusted to 18 (end of emoji)
        let result = safe_extract_snippet(bytes, 0, 19);
        assert!(result.is_ok());
        // The end boundary is adjusted to the character boundary
        assert_eq!(result.unwrap(), "fn test() { // 🎉");
    }

    #[test]
    fn test_safe_extract_snippet_cjk() {
        // "fn test() { // 中文 }" - each CJK char is 3 bytes
        let source = "fn test() { // 中文 }";
        let bytes = source.as_bytes();
        // Extract from start through part of CJK comment
        // "中" is bytes 15-17, "文" is bytes 18-20
        // Byte 18 is at the start of "文", so we get it
        let result = safe_extract_snippet(bytes, 0, 18);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "fn test() { // 中");
    }

    #[test]
    fn test_safe_extract_snippet_boundary_splitting() {
        // "café" - 'é' is bytes [0xC3, 0xA9]
        let source = "fn café()";
        let bytes = source.as_bytes();
        // Try to split in the middle of 'é' (byte 6 is in middle of 2-byte sequence)
        // 'é' is at bytes 5-6 in "fn café"
        let result = safe_extract_snippet(bytes, 0, 6);
        // extract_symbol_content_safe adjusts end to char boundary (byte 5)
        // So we get "fn caf" instead of splitting 'é'
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "fn caf");
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
        let source = "fn test() { return 42; }";
        let bytes = source.as_bytes();
        // Byte 11 is the '{', so [0..11] gives us "fn test() {"
        let result = extract_symbol_content_safe(bytes, 0, 11);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "fn test() {");
    }

    #[test]
    fn test_safe_str_slice() {
        // Test string slice with safe boundaries
        let source = "fn test() { return 42; }";
        // Byte 11 is the '{', so [0..11] gives us "fn test() {"
        let result = safe_str_slice(source, 0, 11);
        // safe_str_slice uses source.get() which returns exact range or None
        assert_eq!(result, Some("fn test() {"));
    }

    #[test]
    fn test_safe_str_slice_multi_byte() {
        // Test with multi-byte characters
        // "café" = c(1) a(1) f(1) é(2) = 5 bytes total
        let source = "café";
        // Byte 0-3 is "caf" (ends before é which starts at byte 3)
        let result = safe_str_slice(source, 0, 3);
        assert_eq!(result, Some("caf"));
    }

    #[test]
    fn test_safe_str_slice_invalid_boundary() {
        // Try to slice in middle of multi-byte character
        let source = "café"; // 'é' is 2 bytes at positions 3-4
                             // Byte 4 is in middle of 'é', so safe_str_slice returns None
        let result = safe_str_slice(source, 0, 4);
        // Should return None since byte 4 is not a valid char boundary
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_symbol_content_safe_adjusts_end_boundary() {
        // Test that extract_symbol_content_safe adjusts end boundary
        let source = "café";
        let bytes = source.as_bytes();
        // Byte 4 is in middle of 'é', function should adjust to byte 3
        let result = extract_symbol_content_safe(bytes, 0, 4);
        // Should return "caf" (adjusted to valid boundary)
        assert_eq!(result, Some("caf".to_string()));
    }

    #[test]
    fn test_extract_symbol_content_safe_start_at_boundary() {
        // Test extraction starting at multi-byte char boundary
        let source = "a🎉b"; // emoji is 4 bytes at positions 1-4
        let bytes = source.as_bytes();
        // Start at position 1 (emoji start), end at 5 (after emoji)
        let result = extract_symbol_content_safe(bytes, 1, 5);
        assert_eq!(result, Some("🎉".to_string()));
    }

    #[test]
    fn test_extract_symbol_content_safe_start_splits_char_returns_none() {
        // Test that start at invalid boundary returns None
        let source = "a🎉b"; // emoji is 4 bytes at positions 1-4
        let bytes = source.as_bytes();
        // Start at position 2 (in middle of emoji) - should return None
        let result = extract_symbol_content_safe(bytes, 2, 5);
        assert_eq!(result, None);
    }
}
