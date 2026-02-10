// Test Rust file for language detection
use std::collections::HashMap;

const TEST_CONSTANT: i32 = 42;

/// Test struct for language detection
#[derive(Debug, Clone)]
pub struct TestStruct {
    pub name: String,
    pub value: i32,
}

impl TestStruct {
    pub fn new(name: String, value: i32) -> Self {
        Self { name, value }
    }

    pub fn test_method(&self, x: i32) -> i32 {
        self.value + x
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }
}

/// Test function
pub fn test_function(x: i32, y: i32) -> i32 {
    x + y
}

/// Generic function
pub fn generic_function<T: std::fmt::Display>(value: T) -> String {
    format!("{}", value)
}

/// Trait definition
pub trait TestTrait {
    fn trait_method(&self) -> String;
}

impl TestTrait for TestStruct {
    fn trait_method(&self) -> String {
        self.name.clone()
    }
}

/// Module-level function
pub fn module_function(items: &[i32]) -> usize {
    items.len()
}

fn main() {
    let test = TestStruct::new("test".to_string(), TEST_CONSTANT);
    println!("Name: {}", test.get_name());
    println!("Result: {}", test.test_method(10));
}
