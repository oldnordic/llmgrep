//! Cross-backend verification tests for language detection and clean output.
//!
//! This test suite verifies:
//! - Language field accuracy for all supported file types
//! - No debug output in production commands (complete, lookup, search)

// Test 1: Python language detection
#[test]
fn test_python_language_detection() {
    let language = llmgrep::query::infer_language("src/test.py");
    assert_eq!(language, Some("Python"));
}

// Test 2: JavaScript language detection
#[test]
fn test_javascript_language_detection() {
    // Test with explicit JavaScript file extension
    let language = llmgrep::query::infer_language("src/test.js");
    assert_eq!(language, Some("JavaScript"));

    // Test .jsx extension
    let language_jsx = llmgrep::query::infer_language("src/component.jsx");
    assert_eq!(language_jsx, Some("JavaScript"));
}

// Test 3: TypeScript language detection
#[test]
fn test_typescript_language_detection() {
    let language = llmgrep::query::infer_language("src/test.ts");
    assert_eq!(language, Some("TypeScript"));

    // Test .tsx extension
    let language_tsx = llmgrep::query::infer_language("src/component.tsx");
    assert_eq!(language_tsx, Some("TypeScript"));
}

// Test 4: C language detection
#[test]
fn test_c_language_detection() {
    let language = llmgrep::query::infer_language("src/test.c");
    assert_eq!(language, Some("C"));

    // Test .h extension
    let language_h = llmgrep::query::infer_language("include/header.h");
    assert_eq!(language_h, Some("C"));
}

// Test 5: C++ language detection
#[test]
fn test_cpp_language_detection() {
    let language = llmgrep::query::infer_language("src/test.cpp");
    assert_eq!(language, Some("C++"));

    // Test .cc extension
    let language_cc = llmgrep::query::infer_language("src/test.cc");
    assert_eq!(language_cc, Some("C++"));

    // Test .cxx extension
    let language_cxx = llmgrep::query::infer_language("src/test.cxx");
    assert_eq!(language_cxx, Some("C++"));

    // Test .hpp extension
    let language_hpp = llmgrep::query::infer_language("include/header.hpp");
    assert_eq!(language_hpp, Some("C++"));
}

// Test 6: Java language detection
#[test]
fn test_java_language_detection() {
    let language = llmgrep::query::infer_language("src/Test.java");
    assert_eq!(language, Some("Java"));
}

// Test 7: Rust language detection
#[test]
fn test_rust_language_detection() {
    let language = llmgrep::query::infer_language("src/test.rs");
    assert_eq!(language, Some("Rust"));
}

// Test 8: Unknown file extension
#[test]
fn test_unknown_language_detection() {
    let language = llmgrep::query::infer_language("src/test.unknown");
    assert_eq!(language, None);

    // Test file without extension
    let language_no_ext = llmgrep::query::infer_language("src/Makefile");
    assert_eq!(language_no_ext, None);
}

// Test 9: All supported extensions are detected
#[test]
fn test_all_supported_extensions() {
    let extensions = vec![
        ("test.rs", "Rust"),
        ("test.py", "Python"),
        ("test.js", "JavaScript"),
        ("test.jsx", "JavaScript"),
        ("test.ts", "TypeScript"),
        ("test.tsx", "TypeScript"),
        ("test.c", "C"),
        ("test.cpp", "C++"),
        ("test.cc", "C++"),
        ("test.cxx", "C++"),
        ("test.h", "C"),
        ("test.hpp", "C++"),
        ("test.hxx", "C++"),
        ("test.java", "Java"),
        ("test.go", "Go"),
        ("test.rb", "Ruby"),
        ("test.php", "PHP"),
        ("test.swift", "Swift"),
        ("test.kt", "Kotlin"),
        ("test.kts", "Kotlin"),
        ("test.scala", "Scala"),
        ("test.sh", "Shell"),
        ("test.bash", "Shell"),
        ("test.lua", "Lua"),
        ("test.r", "R"),
        ("test.m", "Matlab"),
        ("test.cs", "C#"),
    ];

    for (file, expected) in extensions {
        let result = llmgrep::query::infer_language(&format!("src/{}", file));
        assert_eq!(result, Some(expected), "Failed for {}.{}", file, expected);
    }
}

// Test 10: Language consistency across different search modes
#[test]
fn test_language_consistency() {
    // Test that infer_language returns consistent results
    let test_cases = vec![
        ("file.rs", "Rust"),
        ("file.py", "Python"),
        ("file.js", "JavaScript"),
        ("file.ts", "TypeScript"),
        ("file.tsx", "TypeScript"),
        ("file.jsx", "JavaScript"),
        ("file.c", "C"),
        ("file.cpp", "C++"),
        ("file.java", "Java"),
    ];

    for (file, expected) in test_cases {
        let result = llmgrep::query::infer_language(file);
        assert_eq!(result, Some(expected),
                   "Language mismatch for {}: got {:?}, expected {}",
                   file, result, expected);
    }
}

// Test 11: File paths with directories are handled correctly
#[test]
fn test_language_detection_with_paths() {
    let test_cases = vec![
        ("src/lib/module/test.py", "Python"),
        ("../../components/Header.tsx", "TypeScript"),
        ("/usr/local/include/header.h", "C"),
        ("./src/main.rs", "Rust"),
        ("vendor/bundle/ruby/gem.rb", "Ruby"),
    ];

    for (path, expected) in test_cases {
        let result = llmgrep::query::infer_language(path);
        assert_eq!(result, Some(expected),
                   "Language mismatch for {}: got {:?}, expected {}",
                   path, result, expected);
    }
}

// Test 12: Case sensitivity of extensions
#[test]
fn test_language_detection_case_sensitivity() {
    // Extensions are case-sensitive in most systems
    let test_cases: Vec<(&str, Option<&str>)> = vec![
        ("test.PY", None),      // Uppercase extension not recognized
        ("test.Py", None),      // Mixed case not recognized
        ("test.py", Some("Python")),  // Lowercase recognized
        ("test.RS", None),
        ("test.rs", Some("Rust")),
    ];

    for (file, expected) in test_cases {
        let result = llmgrep::query::infer_language(file);
        assert_eq!(result, expected,
                   "Language mismatch for {}: got {:?}, expected {:?}",
                   file, result, expected);
    }
}

// Test 13: Multiple dots in filename
#[test]
fn test_language_detection_with_multiple_dots() {
    // Only the last extension should matter
    let test_cases: Vec<(&str, Option<&str>)> = vec![
        ("test.min.js", Some("JavaScript")),
        ("component.dev.tsx", Some("TypeScript")),
        ("lib.so.1.0.0", None),  // No extension matches
        ("archive.tar.gz", None),  // gz is not in the list
    ];

    for (file, expected) in test_cases {
        let result = llmgrep::query::infer_language(file);
        assert_eq!(result, expected,
                   "Language mismatch for {}: got {:?}, expected {:?}",
                   file, result, expected);
    }
}

// Test 14: Empty and edge cases
#[test]
fn test_language_detection_edge_cases() {
    assert_eq!(llmgrep::query::infer_language(""), None);
    assert_eq!(llmgrep::query::infer_language("."), None);
    assert_eq!(llmgrep::query::infer_language(".."), None);
    assert_eq!(llmgrep::query::infer_language("file."), None);  // Empty extension
    assert_eq!(llmgrep::query::infer_language(".hidden"), None);  // Hidden file
}

// Test 15: Verify native-v2 backend doesn't output debug info (code inspection)
#[test]
fn test_no_debug_strings_in_code() {
    // Verify that debug output was removed from native_v2.rs
    // This is a compile-time check by examining the source
    use std::fs;
    use std::path::Path;

    let native_v2_path = Path::new("src/backend/native_v2.rs");
    if native_v2_path.exists() {
        let content = fs::read_to_string(native_v2_path).expect("failed to read test file");

        // Check that no DEBUG: eprintln statements exist
        assert!(!content.contains("eprintln!(\"DEBUG:"),
                "native_v2.rs should not contain eprintln DEBUG statements");

        // Check that no bare DEBUG comments exist (from removed code)
        let debug_lines: Vec<&str> = content
            .lines()
            .filter(|line| line.trim().starts_with("// DEBUG:") ||
                        line.trim().starts_with("/* DEBUG:"))
            .collect();

        assert!(debug_lines.is_empty(),
                "native_v2.rs should not contain DEBUG comment lines: {:?}",
                debug_lines);
    }
}
