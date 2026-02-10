/* Test C file for language detection */
#include <stdio.h>
#include <stdlib.h>

#define TEST_CONSTANT 42

typedef struct {
    int value;
    char* name;
} TestStruct;

/* Test function declaration */
int test_function(int x, int y);

/* Test function implementation */
int test_function(int x, int y) {
    return x + y;
}

/* Test macro */
#define MAX(a, b) ((a) > (b) ? (a) : (b))

/* Global variable */
static int global_value = 0;

int main(void) {
    TestStruct s = { .value = 10, .name = "test" };
    printf("Value: %d\n", test_function(s.value, TEST_CONSTANT));
    return 0;
}
