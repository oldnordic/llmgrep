// Test C++ file for language detection
#include <iostream>
#include <vector>
#include <string>

const int TEST_CONSTANT = 42;

class TestClass {
private:
    std::string name;
    int value;

public:
    TestClass(const std::string& n, int v) : name(n), value(v) {}

    // Test method
    int testMethod(int x) const {
        return value + x;
    }

    // Getter
    std::string getName() const { return name; }

    // Virtual method
    virtual void print() const {
        std::cout << name << ": " << value << std::endl;
    }
};

// Derived class
class DerivedTestClass : public TestClass {
public:
    DerivedTestClass(const std::string& n, int v) : TestClass(n, v) {}

    void print() const override {
        std::cout << "Derived: " << getName() << std::endl;
    }
};

// Template function
template<typename T>
T test_function(T a, T b) {
    return a < b ? a : b;
}

// Namespace
namespace TestNamespace {
    int namespace_function(int x) {
        return x * 2;
    }
}

int main() {
    TestClass obj("test", TEST_CONSTANT);
    obj.print();
    return 0;
}
