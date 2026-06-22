use crate::cli::{find_git_root_db, resolve_db_path, validate_path, Cli, Command, SearchMode};
use clap::Parser;
use llmgrep::error::LlmError;
use llmgrep::output::OutputFormat;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static CWD_MUTEX: Mutex<()> = Mutex::new(());

fn create_temp_db() -> std::io::Result<PathBuf> {
    let temp_file = std::env::temp_dir().join(format!("llmgrep_test_{}.db", std::process::id()));
    std::fs::File::create(&temp_file)?;
    Ok(temp_file)
}

#[test]
fn test_basic_search_command() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
    ];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Should parse basic search command");
    let cli = result.unwrap();
    assert_eq!(
        cli.db.as_ref().unwrap().to_str().unwrap(),
        temp_db.to_str().unwrap()
    );
    match cli.command {
        Some(Command::Search { query, .. }) => {
            assert_eq!(query, "test");
        }
        _ => panic!("Expected Command::Search"),
    }
}

#[test]
fn test_invalid_flag() {
    let args = ["llmgrep", "--invalid-flag", "search", "--query", "test"];
    let result = Cli::try_parse_from(args);
    assert!(result.is_err(), "Should reject unknown flag");
}

#[test]
fn test_limit_validation_zero() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
        "--limit",
        "0",
    ];
    let result = Cli::try_parse_from(args);
    assert!(result.is_err(), "Should reject limit=0 (range is 1..=1000)");
}

#[test]
fn test_limit_valid() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
        "--limit",
        "500",
    ];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Should accept valid limit");
    let cli = result.unwrap();
    match cli.command {
        Some(Command::Search { limit, .. }) => {
            assert_eq!(limit, 500);
        }
        _ => panic!("Expected Command::Search"),
    }
}

#[test]
fn test_regex_mode() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test.*",
        "--regex",
    ];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Should parse regex flag");
    let cli = result.unwrap();
    match cli.command {
        Some(Command::Search { regex, .. }) => {
            assert!(regex, "Regex flag should be set");
        }
        _ => panic!("Expected Command::Search"),
    }
}

#[test]
fn test_field_parsing() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
        "--fields",
        "context,snippet,score",
    ];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Should parse fields");
    let cli = result.unwrap();
    match cli.command {
        Some(Command::Search { fields, .. }) => {
            assert_eq!(fields.as_ref().unwrap(), "context,snippet,score");
        }
        _ => panic!("Expected Command::Search"),
    }
}

#[test]
fn test_search_mode_symbols() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
        "--mode",
        "symbols",
    ];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Should parse symbols mode");
    let cli = result.unwrap();
    match cli.command {
        Some(Command::Search { mode, .. }) => {
            assert!(matches!(mode, SearchMode::Symbols));
        }
        _ => panic!("Expected Command::Search"),
    }
}

#[test]
fn test_search_mode_references() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
        "--mode",
        "references",
    ];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Should parse references mode");
    let cli = result.unwrap();
    match cli.command {
        Some(Command::Search { mode, .. }) => {
            assert!(matches!(mode, SearchMode::References));
        }
        _ => panic!("Expected Command::Search"),
    }
}

#[test]
fn test_search_mode_calls() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
        "--mode",
        "calls",
    ];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Should parse calls mode");
    let cli = result.unwrap();
    match cli.command {
        Some(Command::Search { mode, .. }) => {
            assert!(matches!(mode, SearchMode::Calls));
        }
        _ => panic!("Expected Command::Search"),
    }
}

#[test]
fn test_search_mode_auto() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
        "--mode",
        "auto",
    ];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Should parse auto mode");
    let cli = result.unwrap();
    match cli.command {
        Some(Command::Search { mode, .. }) => {
            assert!(matches!(mode, SearchMode::Auto));
        }
        _ => panic!("Expected Command::Search"),
    }
}

#[test]
fn test_empty_query_accepted_by_clap() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "",
    ];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Clap should accept empty query string");
    let cli = result.unwrap();
    match cli.command {
        Some(Command::Search { query, .. }) => {
            assert_eq!(query, "");
        }
        _ => panic!("Expected Command::Search"),
    }
}

#[test]
fn test_output_format_human() {
    let args = ["llmgrep", "--output", "human", "search", "--query", "test"];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Should parse human output format");
    let cli = result.unwrap();
    assert!(matches!(cli.output, OutputFormat::Human));
}

#[test]
fn test_output_format_json() {
    let args = ["llmgrep", "--output", "json", "search", "--query", "test"];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Should parse json output format");
    let cli = result.unwrap();
    assert!(matches!(cli.output, OutputFormat::Json));
}

#[test]
fn test_output_format_pretty() {
    let args = ["llmgrep", "--output", "pretty", "search", "--query", "test"];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Should parse pretty output format");
    let cli = result.unwrap();
    assert!(matches!(cli.output, OutputFormat::Pretty));
}

#[test]
fn test_candidates_validation_min() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
        "--candidates",
        "0",
    ];
    let result = Cli::try_parse_from(args);
    assert!(
        result.is_err(),
        "Should reject candidates=0 (range is 1..=10000)"
    );
}

#[test]
fn test_candidates_validation_max() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
        "--candidates",
        "10001",
    ];
    let result = Cli::try_parse_from(args);
    assert!(
        result.is_err(),
        "Should reject candidates=10001 (range is 1..=10000)"
    );
}

#[test]
fn test_max_snippet_bytes_validation_min() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
        "--max-snippet-bytes",
        "0",
    ];
    let result = Cli::try_parse_from(args);
    assert!(
        result.is_err(),
        "Should reject max_snippet_bytes=0 (range is 1..=1MB)"
    );
}

#[test]
fn test_context_lines_validation_min() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
        "--context-lines",
        "0",
    ];
    let result = Cli::try_parse_from(args);
    assert!(
        result.is_err(),
        "Should reject context_lines=0 (range is 1..=100)"
    );
}

#[test]
fn test_context_lines_validation_max() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
        "--context-lines",
        "101",
    ];
    let result = Cli::try_parse_from(args);
    assert!(
        result.is_err(),
        "Should reject context_lines=101 (range is 1..=100)"
    );
}

#[test]
fn test_max_context_lines_validation_min() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
        "--max-context-lines",
        "0",
    ];
    let result = Cli::try_parse_from(args);
    assert!(
        result.is_err(),
        "Should reject max_context_lines=0 (range is 1..=500)"
    );
}

#[test]
fn test_max_context_lines_validation_max() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
        "--max-context-lines",
        "501",
    ];
    let result = Cli::try_parse_from(args);
    assert!(
        result.is_err(),
        "Should reject max_context_lines=501 (range is 1..=500)"
    );
}

#[test]
fn test_path_validation_sensitive_etc() {
    let path = Path::new("/etc/passwd");
    let result = validate_path(path, true);
    assert!(result.is_err(), "Should reject /etc/passwd");
    match result {
        Err(LlmError::PathValidationFailed { reason, .. }) => {
            assert!(
                reason.contains("not allowed"),
                "Error should mention access denied"
            );
        }
        _ => panic!("Expected PathValidationFailed error"),
    }
}

#[test]
fn test_path_validation_var_tmp() {
    let path = Path::new("/var/tmp/test");
    let result = validate_path(path, false);
    assert!(result.is_err(), "Should reject /var/tmp/test");
    match result {
        Err(LlmError::PathValidationFailed { .. }) => {}
        _ => panic!("Expected PathValidationFailed error for /var/tmp"),
    }
}

#[test]
fn test_path_validation_allowed_path() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let result = validate_path(&temp_db, true);
    assert!(result.is_ok(), "Should allow temp db path");
    let canonical = result.unwrap();
    assert!(canonical.exists(), "Validated path should exist");
}

#[test]
fn test_ast_command_basic() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let temp_file = std::env::temp_dir().join("test_ast.rs");
    std::fs::write(&temp_file, "fn main() {}").expect("Failed to create temp file");

    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "ast",
        "--file",
        temp_file.to_str().unwrap(),
    ];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Should parse ast command");

    let cli = result.unwrap();
    match cli.command {
        Some(Command::Ast {
            file,
            position,
            limit,
        }) => {
            assert_eq!(file, temp_file);
            assert_eq!(position, None);
            assert_eq!(limit, 10000);
        }
        _ => panic!("Expected Command::Ast"),
    }

    std::fs::remove_file(&temp_file).ok();
}

#[test]
fn test_ast_command_with_position() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let temp_file = std::env::temp_dir().join("test_ast.rs");
    std::fs::write(&temp_file, "fn main() {}").expect("Failed to create temp file");

    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "ast",
        "--file",
        temp_file.to_str().unwrap(),
        "--position",
        "100",
    ];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Should parse ast command with position");

    let cli = result.unwrap();
    match cli.command {
        Some(Command::Ast { position, .. }) => {
            assert_eq!(position, Some(100));
        }
        _ => panic!("Expected Command::Ast"),
    }

    std::fs::remove_file(&temp_file).ok();
}

#[test]
fn test_ast_command_with_limit() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let temp_file = std::env::temp_dir().join("test_ast.rs");
    std::fs::write(&temp_file, "fn main() {}").expect("Failed to create temp file");

    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "ast",
        "--file",
        temp_file.to_str().unwrap(),
        "--limit",
        "500",
    ];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Should parse ast command with limit");

    let cli = result.unwrap();
    match cli.command {
        Some(Command::Ast { limit, .. }) => {
            assert_eq!(limit, 500);
        }
        _ => panic!("Expected Command::Ast"),
    }

    std::fs::remove_file(&temp_file).ok();
}

#[test]
fn test_ast_limit_validation_min() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let temp_file = std::env::temp_dir().join("test_ast.rs");
    std::fs::write(&temp_file, "fn main() {}").expect("Failed to create temp file");

    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "ast",
        "--file",
        temp_file.to_str().unwrap(),
        "--limit",
        "0",
    ];
    let result = Cli::try_parse_from(args);
    assert!(
        result.is_err(),
        "Should reject limit=0 (range is 1..=100000)"
    );

    std::fs::remove_file(&temp_file).ok();
}

#[test]
fn test_ast_limit_validation_max() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let temp_file = std::env::temp_dir().join("test_ast.rs");
    std::fs::write(&temp_file, "fn main() {}").expect("Failed to create temp file");

    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "ast",
        "--file",
        temp_file.to_str().unwrap(),
        "--limit",
        "100001",
    ];
    let result = Cli::try_parse_from(args);
    assert!(
        result.is_err(),
        "Should reject limit=100001 (range is 1..=100000)"
    );

    std::fs::remove_file(&temp_file).ok();
}

#[test]
fn test_find_ast_command_basic() {
    let temp_db = create_temp_db().expect("Failed to create temp db");

    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "find-ast",
        "--kind",
        "function_item",
    ];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Should parse find-ast command");

    let cli = result.unwrap();
    match cli.command {
        Some(Command::FindAst { kind }) => {
            assert_eq!(kind, "function_item");
        }
        _ => panic!("Expected Command::FindAst"),
    }
}

#[test]
fn test_find_ast_command_with_various_kinds() {
    let temp_db = create_temp_db().expect("Failed to create temp db");

    let test_kinds = [
        "if_expression",
        "while_expression",
        "for_expression",
        "struct_item",
    ];

    for kind in test_kinds {
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "find-ast",
            "--kind",
            kind,
        ];
        let result = Cli::try_parse_from(args);
        assert!(result.is_ok(), "Should parse find-ast with kind {}", kind);

        let cli = result.unwrap();
        match cli.command {
            Some(Command::FindAst { kind: k }) => {
                assert_eq!(k, kind);
            }
            _ => panic!("Expected Command::FindAst"),
        }
    }
}

#[test]
fn test_resolve_db_path_explicit_flag() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
    ];
    let cli = Cli::try_parse_from(args).expect("parse");
    let result = resolve_db_path(&cli);
    assert!(result.is_ok(), "Explicit --db should resolve: {:?}", result);
    let _ = std::fs::remove_file(&temp_db);
}

#[test]
fn test_resolve_db_path_missing_flag_no_fallback() {
    let _guard = CWD_MUTEX.lock().expect("cwd mutex");
    let temp_dir = std::env::temp_dir().join(format!("llmgrep_no_db_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("get cwd");
    let _ = std::env::set_current_dir(&temp_dir);
    let args = ["llmgrep", "search", "--query", "test"];
    let cli = Cli::try_parse_from(args).expect("parse");
    let result = resolve_db_path(&cli);
    let _ = std::env::set_current_dir(&orig_cwd);
    let _ = std::fs::remove_dir(&temp_dir);
    assert!(
        result.is_err(),
        "Should fail when no --db and no .magellan/llmgrep.db in CWD: {:?}",
        result
    );
}

#[test]
fn test_resolve_db_path_fallback_from_cwd() {
    let _guard = CWD_MUTEX.lock().expect("cwd mutex");
    let temp_dir =
        std::env::temp_dir().join(format!("llmgrep_resolve_test_{}", std::process::id()));
    let magellan_dir = temp_dir.join(".magellan");
    std::fs::create_dir_all(&magellan_dir).expect("create .magellan");
    let db_file = magellan_dir.join("llmgrep.db");
    std::fs::File::create(&db_file).expect("create db file");

    let orig_cwd = std::env::current_dir().expect("get cwd");
    let _ = std::env::set_current_dir(&temp_dir);
    let args = ["llmgrep", "search", "--query", "test"];
    let cli = Cli::try_parse_from(args).expect("parse");
    let result = resolve_db_path(&cli);
    let _ = std::env::set_current_dir(&orig_cwd);

    let _ = std::fs::remove_dir_all(&temp_dir);
    assert!(
        result.is_ok(),
        "Should find .magellan/llmgrep.db in CWD: {:?}",
        result
    );
}

#[test]
fn test_find_git_root_db_returns_none_without_git() {
    let temp_dir = std::env::temp_dir().join(format!("llmgrep_no_git_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("create dir");
    let result = find_git_root_db(&temp_dir);
    let _ = std::fs::remove_dir(&temp_dir);
    assert!(result.is_none(), "Should return None without .git");
}

#[test]
fn test_find_git_root_db_finds_git_root() {
    let temp_dir = std::env::temp_dir().join(format!("llmgrep_git_test_{}", std::process::id()));
    let sub_dir = temp_dir.join("src").join("module");
    std::fs::create_dir_all(&sub_dir).expect("create sub dir");
    std::fs::create_dir_all(temp_dir.join(".git")).expect("create .git");

    let result = find_git_root_db(&sub_dir);
    let _ = std::fs::remove_dir_all(&temp_dir);
    assert!(result.is_some(), "Should find git root from nested dir");
    let path = result.unwrap();
    assert!(
        path.ends_with("llmgrep.db"),
        "Path should end with llmgrep.db: {:?}",
        path
    );
}

#[test]
fn test_truncate_response_helper() {
    let items = vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string(), "e".to_string()];
    let format_fn = |slice: &[String]| slice.join(" ");

    let (pruned, _tokens, truncated) = crate::display::truncate_response(items.clone(), None, format_fn);
    assert_eq!(pruned.len(), 5);
    assert!(!truncated);

    let (pruned, _tokens, truncated) = crate::display::truncate_response(items.clone(), Some(0), format_fn);
    assert_eq!(pruned.len(), 5);
    assert!(!truncated);

    let (pruned, tokens, truncated) = crate::display::truncate_response(items.clone(), Some(1), format_fn);
    assert_eq!(pruned.len(), 2);
    assert_eq!(pruned, vec!["a".to_string(), "b".to_string()]);
    assert!(truncated);
    assert!(tokens.unwrap() <= 1);
}

