// Test Java file for language detection
package com.test.example;

import java.util.List;
import java.util.ArrayList;

/**
 * Test class for language detection.
 */
public class Test {
    private static final int TEST_CONSTANT = 42;
    private String name;
    private int value;

    public Test(String name, int value) {
        this.name = name;
        this.value = value;
    }

    /**
     * Test method.
     * @param x input value
     * @return result
     */
    public int testMethod(int x) {
        return value + x;
    }

    public String getName() {
        return name;
    }

    // Generic method
    public <T extends Number> T genericMethod(T value) {
        return value;
    }

    // Static method
    public static int staticFunction(int x, int y) {
        return x + y;
    }

    // Varargs method
    public int sumAll(int... values) {
        int sum = 0;
        for (int v : values) {
            sum += v;
        }
        return sum;
    }

    public static void main(String[] args) {
        Test obj = new Test("test", TEST_CONSTANT);
        System.out.println(obj.getName() + ": " + obj.testMethod(10));
    }
}

// Interface
interface TestInterface {
    void interfaceMethod();
}

// Enum
enum TestEnum {
    VALUE1,
    VALUE2,
    VALUE3
}
