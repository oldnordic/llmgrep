// Test JavaScript file for language detection
class TestClass {
    constructor(name) {
        this.name = name;
    }

    testMethod(value) {
        if (value > 0) {
            return this.name;
        }
        return null;
    }
}

function testFunction(items) {
    return items.length;
}

// Module-level constant
const TEST_CONSTANT = "test_value";

// Arrow function
const arrowFunction = (x, y) => x + y;

// Export for module systems
module.exports = { TestClass, testFunction };
