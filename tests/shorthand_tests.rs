//! Shorthand expansion and multi-language AST filtering tests.
//!
//! Tests for:
//! - Shorthand expansion (loops, conditionals, functions, etc.)
//! - Multiple shorthand handling with deduplication
//! - Mixed shorthand and specific kind combinations
//! - Language-specific node kind expansion
//! - Unknown shorthand passthrough

use llmgrep::ast::{
    expand_shorthand, expand_shorthand_with_language, expand_shorthands,
    get_node_kinds_for_language, get_supported_languages, AST_SHORTHANDS,
};

#[test]
fn test_expand_shorthand_loops() {
    let result = expand_shorthand("loops");
    assert_eq!(result, "for_expression,while_expression,loop_expression");
}

#[test]
fn test_expand_shorthand_conditionals() {
    let result = expand_shorthand("conditionals");
    assert_eq!(
        result,
        "if_expression,match_expression,match_arm"
    );
}

#[test]
fn test_expand_shorthand_functions() {
    let result = expand_shorthand("functions");
    assert_eq!(result, "function_item,closure_expression,async_function_item");
}

#[test]
fn test_expand_shorthand_declarations() {
    let result = expand_shorthand("declarations");
    assert_eq!(
        result,
        "struct_item,enum_item,let_declaration,const_item,static_item,type_alias_item"
    );
}

#[test]
fn test_expand_shorthand_unsafe() {
    let result = expand_shorthand("unsafe");
    assert_eq!(result, "unsafe_block");
}

#[test]
fn test_expand_shorthand_types() {
    let result = expand_shorthand("types");
    assert_eq!(result, "struct_item,enum_item,type_alias_item,union_item");
}

#[test]
fn test_expand_shorthand_macros() {
    let result = expand_shorthand("macros");
    assert_eq!(result, "macro_invocation,macro_definition,macro_rule");
}

#[test]
fn test_expand_shorthand_mods() {
    let result = expand_shorthand("mods");
    assert_eq!(result, "mod_item");
}

#[test]
fn test_expand_shorthand_traits() {
    let result = expand_shorthand("traits");
    assert_eq!(result, "trait_item,trait_impl_item");
}

#[test]
fn test_expand_shorthand_impls() {
    let result = expand_shorthand("impls");
    assert_eq!(result, "impl_item");
}

#[test]
fn test_unknown_shorthand_passthrough() {
    let result = expand_shorthand("unknown_kind");
    assert_eq!(result, "unknown_kind");
}

#[test]
fn test_specific_node_kind_passthrough() {
    let result = expand_shorthand("function_item");
    assert_eq!(result, "function_item");
}

#[test]
fn test_expand_shorthands_single() {
    let result = expand_shorthands("loops");
    assert_eq!(result.len(), 3);
    assert!(result.contains(&"for_expression".to_string()));
    assert!(result.contains(&"while_expression".to_string()));
    assert!(result.contains(&"loop_expression".to_string()));
}

#[test]
fn test_expand_shorthands_multiple() {
    let result = expand_shorthands("loops,conditionals");
    // Should contain 6 unique kinds (3 loops + 3 conditionals)
    assert_eq!(result.len(), 6);
    assert!(result.contains(&"for_expression".to_string()));
    assert!(result.contains(&"if_expression".to_string()));
    assert!(result.contains(&"match_expression".to_string()));
}

#[test]
fn test_expand_shorthands_deduplication() {
    // "loops,loops" should not duplicate
    let result = expand_shorthands("loops,loops");
    assert_eq!(result.len(), 3);
}

#[test]
fn test_expand_shorthands_mixed_shorthand_specific() {
    let result = expand_shorthands("loops,function_item");
    assert!(result.contains(&"for_expression".to_string()));
    assert!(result.contains(&"function_item".to_string()));
    assert!(result.len() >= 4); // At least 3 loops + 1 function_item
}

#[test]
fn test_expand_shorthands_comma_separated_specific() {
    let result = expand_shorthands("for_expression,while_expression");
    assert_eq!(result.len(), 2);
    assert!(result.contains(&"for_expression".to_string()));
    assert!(result.contains(&"while_expression".to_string()));
}

#[test]
fn test_expand_shorthands_empty_input() {
    let result = expand_shorthands("");
    assert!(result.is_empty());
}

#[test]
fn test_expand_shorthands_whitespace_handling() {
    let result = expand_shorthands(" loops , conditionals ");
    assert_eq!(result.len(), 6);
}

#[test]
fn test_get_supported_languages() {
    let languages = get_supported_languages();
    assert_eq!(languages.len(), 4);
    assert!(languages.contains(&"rust"));
    assert!(languages.contains(&"python"));
    assert!(languages.contains(&"javascript"));
    assert!(languages.contains(&"typescript"));
}

#[test]
fn test_python_function_kinds() {
    let result = get_node_kinds_for_language("python", "functions");
    assert!(result.is_some());
    let kinds = result.unwrap();
    assert!(kinds.contains(&"function_definition".to_string()));
    assert!(kinds.contains(&"lambda".to_string()));
    assert!(kinds.contains(&"async_function_definition".to_string()));
}

#[test]
fn test_python_loops_kinds() {
    let result = get_node_kinds_for_language("python", "loops");
    assert!(result.is_some());
    let kinds = result.unwrap();
    assert!(kinds.contains(&"for_statement".to_string()));
    assert!(kinds.contains(&"while_statement".to_string()));
}

#[test]
fn test_python_conditionals_kinds() {
    let result = get_node_kinds_for_language("python", "conditionals");
    assert!(result.is_some());
    let kinds = result.unwrap();
    assert!(kinds.contains(&"if_statement".to_string()));
    assert!(kinds.contains(&"match_statement".to_string()));
}

#[test]
fn test_python_declarations_kinds() {
    let result = get_node_kinds_for_language("python", "declarations");
    assert!(result.is_some());
    let kinds = result.unwrap();
    assert!(kinds.contains(&"class_definition".to_string()));
    assert!(kinds.contains(&"type_alias_statement".to_string()));
}

#[test]
fn test_javascript_function_kinds() {
    let result = get_node_kinds_for_language("javascript", "functions");
    assert!(result.is_some());
    let kinds = result.unwrap();
    assert!(kinds.contains(&"function_declaration".to_string()));
    assert!(kinds.contains(&"arrow_function".to_string()));
}

#[test]
fn test_javascript_loops_kinds() {
    let result = get_node_kinds_for_language("javascript", "loops");
    assert!(result.is_some());
    let kinds = result.unwrap();
    assert!(kinds.contains(&"for_statement".to_string()));
    assert!(kinds.contains(&"while_statement".to_string()));
    assert!(kinds.contains(&"for_of_statement".to_string()));
}

#[test]
fn test_typescript_function_kinds() {
    let result = get_node_kinds_for_language("typescript", "functions");
    assert!(result.is_some());
    let kinds = result.unwrap();
    assert!(kinds.contains(&"function_declaration".to_string()));
    assert!(kinds.contains(&"arrow_function".to_string()));
}

#[test]
fn test_typescript_declarations_kinds() {
    let result = get_node_kinds_for_language("typescript", "declarations");
    assert!(result.is_some());
    let kinds = result.unwrap();
    assert!(kinds.contains(&"interface_declaration".to_string()));
    assert!(kinds.contains(&"enum_declaration".to_string()));
}

#[test]
fn test_unknown_language_returns_none() {
    let result = get_node_kinds_for_language("unknown", "functions");
    assert!(result.is_none());
}

#[test]
fn test_unknown_category_returns_none() {
    let result = get_node_kinds_for_language("python", "unknown");
    assert!(result.is_none());
}

#[test]
fn test_expand_shorthand_with_language_python() {
    let result = expand_shorthand_with_language("functions", Some("python"));
    assert!(result.contains(&"function_definition".to_string()));
    assert!(result.contains(&"lambda".to_string()));
}

#[test]
fn test_expand_shorthand_with_language_javascript() {
    let result = expand_shorthand_with_language("loops", Some("javascript"));
    assert!(result.contains(&"for_statement".to_string()));
    assert!(result.contains(&"while_statement".to_string()));
}

#[test]
fn test_expand_shorthand_with_language_typescript() {
    let result = expand_shorthand_with_language("loops", Some("typescript"));
    assert!(result.contains(&"for_statement".to_string()));
    assert!(result.contains(&"while_statement".to_string()));
}

#[test]
fn test_expand_shorthand_with_language_no_language() {
    // Without language, should fall back to Rust shorthands
    let result = expand_shorthand_with_language("loops", None);
    assert!(result.contains(&"for_expression".to_string()));
    assert!(result.contains(&"while_expression".to_string()));
}

#[test]
fn test_expand_shorthand_with_language_unknown_language() {
    // Unknown language should fall back to Rust shorthands
    let result = expand_shorthand_with_language("loops", Some("unknown"));
    assert!(result.contains(&"for_expression".to_string()));
    assert!(result.contains(&"while_expression".to_string()));
}

#[test]
fn test_case_insensitive_shorthand() {
    let result1 = expand_shorthand("loops");
    let result2 = expand_shorthand("LOOPS");
    let result3 = expand_shorthand("LoOpS");
    assert_eq!(result1, result2);
    assert_eq!(result2, result3);
}

#[test]
fn test_ast_shortands_static_map() {
    // Verify AST_SHORTHANDS contains all expected shorthands
    let shorthand_names: Vec<&str> = AST_SHORTHANDS.iter().map(|(name, _)| *name).collect();

    assert!(shorthand_names.contains(&"loops"));
    assert!(shorthand_names.contains(&"conditionals"));
    assert!(shorthand_names.contains(&"functions"));
    assert!(shorthand_names.contains(&"declarations"));
    assert!(shorthand_names.contains(&"unsafe"));
    assert!(shorthand_names.contains(&"types"));
    assert!(shorthand_names.contains(&"macros"));
    assert!(shorthand_names.contains(&"mods"));
    assert!(shorthand_names.contains(&"traits"));
    assert!(shorthand_names.contains(&"impls"));
}

#[test]
fn test_all_shorthands_have_expansions() {
    for &(shorthand, expansion) in AST_SHORTHANDS {
        let result = expand_shorthand(shorthand);
        assert_eq!(result, expansion);
        assert!(!expansion.is_empty());
    }
}

#[test]
fn test_multi_shorthand_complex_combination() {
    // Test complex real-world combination
    let result = expand_shorthands("loops,conditionals,functions,closure_expression");
    assert!(result.contains(&"for_expression".to_string()));
    assert!(result.contains(&"if_expression".to_string()));
    assert!(result.contains(&"function_item".to_string()));
    assert!(result.contains(&"closure_expression".to_string()));

    // closure_expression should not be duplicated (it's in "functions" shorthand too)
    let closure_count = result.iter().filter(|s| s.as_str() == "closure_expression").count();
    assert_eq!(closure_count, 1);
}
