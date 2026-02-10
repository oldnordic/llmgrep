// Test TypeScript file for language detection
interface TestInterface {
    name: string;
    getValue(): string;
}

class TestClass implements TestInterface {
    private readonly name: string;

    constructor(name: string) {
        this.name = name;
    }

    public getValue(): string {
        return this.name;
    }

    public testMethod(value: number): string | null {
        if (value > 0) {
            return this.name;
        }
        return null;
    }
}

function testFunction(items: string[]): number {
    return items.length;
}

// Module-level constant
const TEST_CONSTANT: string = "test_value";

// Type alias
type StringOrNumber = string | number;

// Generic function
function identity<T>(value: T): T {
    return value;
}

export { TestClass, testFunction, identity, StringOrNumber };
