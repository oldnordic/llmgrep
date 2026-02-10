"""Test Python file for language detection."""
import os
from typing import List, Optional

class TestClass:
    """A test class."""
    def __init__(self, name: str):
        self.name = name

    def test_method(self, value: int) -> Optional[str]:
        """A test method."""
        if value > 0:
            return self.name
        return None

def test_function(items: List[str]) -> int:
    """A test function."""
    return len(items)

# Module-level variable
TEST_CONSTANT = "test_value"
