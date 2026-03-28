#!/bin/bash
# MVP6 Simple Test Script

echo "========================================"
echo "  WinSH MVP6 Simple Test"
echo "========================================"
echo ""

echo "=== Test 1: Built-in Commands ==="
echo "Current directory:"
pwd
echo ""

echo "=== Test 2: Wildcard Expansion ==="
echo "All .toml files:"
echo *.toml
echo ""

echo "=== Test 3: Command Substitution ==="
echo "Current user:"
echo "User: $(whoami)"
echo ""

echo "=== Test 4: Array Operations ==="
echo "Creating test array:"
array define colors red green blue
echo ""

echo "Array elements:"
array get colors 0
array get colors 1
array get colors 2
echo ""

echo "Array length:"
array len colors
echo ""

echo "=== Test 5: Plugin System ==="
echo "Loaded plugins:"
plugin list
echo ""

echo "Available themes:"
theme list
echo ""

echo "=== Test 6: Environment Variables ==="
echo "Setting test variable:"
set MY_VAR=test_value
echo ""

echo "Test complete!"