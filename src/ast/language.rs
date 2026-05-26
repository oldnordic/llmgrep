/// Shorthand mappings for common AST node kind groups.
///
/// These shorthands allow users to query groups of related AST nodes
/// without having to list each kind individually. For example:
/// - `--ast-kind loops` expands to `for_expression,while_expression,loop_expression`
/// - `--ast-kind functions` expands to `function_item,closure_expression`
///
/// # Rust Shorthands
///
/// These are the node kinds for Rust code (tree-sitter-rust):
pub static AST_SHORTHANDS: &[(&str, &str)] = &[
    ("loops", "for_expression,while_expression,loop_expression"),
    ("conditionals", "if_expression,match_expression,match_arm"),
    (
        "functions",
        "function_item,closure_expression,async_function_item",
    ),
    (
        "declarations",
        "struct_item,enum_item,let_declaration,const_item,static_item,type_alias_item",
    ),
    ("unsafe", "unsafe_block"),
    ("types", "struct_item,enum_item,type_alias_item,union_item"),
    ("macros", "macro_invocation,macro_definition,macro_rule"),
    ("mods", "mod_item"),
    ("traits", "trait_item,trait_impl_item"),
    ("impls", "impl_item"),
];

/// Language-specific node kind mappings for shorthands.
///
/// Each language has its own set of AST node kinds from tree-sitter grammars.
/// This structure maps shorthand names like "loops", "functions", etc. to
/// language-specific node kinds.
#[derive(Debug, Clone)]
pub struct LanguageNodeKinds {
    /// Language identifier (rust, python, javascript, typescript)
    pub language: &'static str,
    /// Loop constructs
    pub loops: &'static [&'static str],
    /// Conditional/branching constructs
    pub conditionals: &'static [&'static str],
    /// Functions and callable definitions
    pub functions: &'static [&'static str],
    /// Type/declaration constructs
    pub declarations: &'static [&'static str],
}

/// Node kind mappings for Python (tree-sitter-python)
pub static PYTHON_NODE_KINDS: LanguageNodeKinds = LanguageNodeKinds {
    language: "python",
    loops: &["for_statement", "while_statement"],
    conditionals: &["if_statement", "match_statement"],
    functions: &["function_definition", "lambda", "async_function_definition"],
    declarations: &["class_definition", "type_alias_statement"],
};

/// Node kind mappings for JavaScript (tree-sitter-javascript)
pub static JAVASCRIPT_NODE_KINDS: LanguageNodeKinds = LanguageNodeKinds {
    language: "javascript",
    loops: &[
        "for_statement",
        "for_in_statement",
        "for_of_statement",
        "while_statement",
        "do_statement",
    ],
    conditionals: &["if_statement", "switch_statement", "catch_clause"],
    functions: &[
        "function_declaration",
        "function_expression",
        "arrow_function",
        "generator_function_declaration",
        "generator_function_expression",
    ],
    declarations: &[
        "class_declaration",
        "class_expression",
        "variable_declaration",
        "type_alias_declaration",
    ],
};

/// Node kind mappings for TypeScript (tree-sitter-typescript)
pub static TYPESCRIPT_NODE_KINDS: LanguageNodeKinds = LanguageNodeKinds {
    language: "typescript",
    loops: &[
        "for_statement",
        "for_in_statement",
        "for_of_statement",
        "while_statement",
        "do_statement",
    ],
    conditionals: &["if_statement", "switch_statement", "catch_clause"],
    functions: &[
        "function_declaration",
        "function_expression",
        "arrow_function",
        "generator_function_declaration",
        "generator_function_expression",
    ],
    declarations: &[
        "class_declaration",
        "class_expression",
        "variable_declaration",
        "type_alias_declaration",
        "interface_declaration",
        "enum_declaration",
    ],
};

/// Get all supported languages for AST node kind expansion.
///
/// Returns a slice of language identifiers that have specific node kind mappings.
pub fn get_supported_languages() -> &'static [&'static str] {
    &["rust", "python", "javascript", "typescript"]
}

/// Get node kinds for a specific language and shorthand category.
///
/// # Arguments
///
/// * `language` - Language identifier (rust, python, javascript, typescript)
/// * `category` - Shorthand category (loops, conditionals, functions, declarations)
///
/// # Returns
///
/// * `Some(kinds)` - Slice of node kind strings for the category
/// * `None` - Language or category not found
///
/// # Example
///
/// ```
/// use llmgrep::ast::get_node_kinds_for_language;
///
/// let python_funcs = get_node_kinds_for_language("python", "functions");
/// assert!(python_funcs.is_some());
/// assert!(python_funcs.unwrap().iter().any(|s| s == "function_definition"));
/// ```
pub fn get_node_kinds_for_language(language: &str, category: &str) -> Option<Vec<String>> {
    let kinds = match language.to_lowercase().as_str() {
        "python" => {
            let mapping = &PYTHON_NODE_KINDS;
            match category.to_lowercase().as_str() {
                "loops" => mapping.loops,
                "conditionals" => mapping.conditionals,
                "functions" => mapping.functions,
                "declarations" => mapping.declarations,
                _ => return None,
            }
        }
        "javascript" => {
            let mapping = &JAVASCRIPT_NODE_KINDS;
            match category.to_lowercase().as_str() {
                "loops" => mapping.loops,
                "conditionals" => mapping.conditionals,
                "functions" => mapping.functions,
                "declarations" => mapping.declarations,
                _ => return None,
            }
        }
        "typescript" => {
            let mapping = &TYPESCRIPT_NODE_KINDS;
            match category.to_lowercase().as_str() {
                "loops" => mapping.loops,
                "conditionals" => mapping.conditionals,
                "functions" => mapping.functions,
                "declarations" => mapping.declarations,
                _ => return None,
            }
        }
        _ => return None,
    };
    Some(kinds.iter().map(|s| s.to_string()).collect())
}

/// Expand a single shorthand to its full node kind list.
///
/// If the input is a known shorthand (like "loops", "functions"), returns
/// the expanded comma-separated list. Otherwise, returns the input as-is
/// (it might be a specific node kind like "function_item").
///
/// # Arguments
///
/// * `input` - Shorthand or specific node kind
///
/// # Returns
///
/// Expanded node kinds as a comma-separated string
///
/// # Example
///
/// ```
/// use llmgrep::ast::expand_shorthand;
///
/// assert_eq!(expand_shorthand("loops"), "for_expression,while_expression,loop_expression");
/// assert_eq!(expand_shorthand("function_item"), "function_item"); // Not a shorthand, passed through
/// ```
pub fn expand_shorthand(input: &str) -> String {
    let normalized = input.trim().to_lowercase();
    for &(shorthand, expansion) in AST_SHORTHANDS {
        if normalized == shorthand {
            return expansion.to_string();
        }
    }
    // Not a shorthand, return as-is (might be a specific node kind)
    input.to_string()
}

/// Expand multiple shorthands from a comma-separated input.
///
/// Splits the input by commas, expands each part, and returns a deduplicated
/// list of node kinds. This allows combining shorthands with specific kinds:
/// `loops,function_item` expands to all loop kinds plus `function_item`.
///
/// # Arguments
///
/// * `input` - Comma-separated shorthands and/or specific node kinds
///
/// # Returns
///
/// Deduplicated vector of expanded node kind strings
///
/// # Example
///
/// ```
/// use llmgrep::ast::expand_shorthands;
///
/// let kinds = expand_shorthands("loops,conditionals");
/// assert!(kinds.contains(&"for_expression".to_string()));
/// assert!(kinds.contains(&"if_expression".to_string()));
///
/// let mixed = expand_shorthands("loops,function_item");
/// assert!(mixed.contains(&"for_expression".to_string()));
/// assert!(mixed.contains(&"function_item".to_string()));
/// ```
pub fn expand_shorthands(input: &str) -> Vec<String> {
    let mut result = std::collections::HashSet::new();

    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let expanded = expand_shorthand(part);
        // The expanded result might itself be comma-separated
        for kind in expanded.split(',') {
            let kind = kind.trim();
            if !kind.is_empty() {
                result.insert(kind.to_string());
            }
        }
    }

    // Convert to sorted Vec for consistent ordering
    let mut kinds: Vec<String> = result.into_iter().collect();
    kinds.sort();
    kinds
}

/// Expand a shorthand with language-aware node kind selection.
///
/// When a language is specified, returns language-specific node kinds
/// for the given shorthand. Falls back to Rust shorthands if language
/// is not recognized.
///
/// # Arguments
///
/// * `shorthand` - Shorthand name (loops, functions, etc.)
/// * `language` - Optional language identifier
///
/// # Returns
///
/// Vector of expanded node kind strings
pub fn expand_shorthand_with_language(shorthand: &str, language: Option<&str>) -> Vec<String> {
    let normalized = shorthand.trim().to_lowercase();

    if let Some(lang) = language {
        let lang_lower = lang.to_lowercase();

        // Check if this is a known category that has language-specific mappings
        if let Some(kinds) = get_node_kinds_for_language(&lang_lower, &normalized) {
            return kinds;
        }
    }

    // Fall back to Rust shorthands
    let expanded = expand_shorthand(&normalized);
    expanded.split(',').map(|s| s.trim().to_string()).collect()
}
