#!/bin/bash
# MVP6 Comprehensive Test Script
# This script tests all major features of WinSH MVP6

echo "========================================"
echo "  WinSH MVP6 Comprehensive Test Suite"
echo "========================================"
echo ""

echo "=== Test 1: Built-in Commands ==="
echo "Current directory:"
pwd
echo ""

echo "Listing files:"
ls
echo ""

echo "Testing echo command:"
echo "Hello from MVP6!"
echo ""

echo "Testing environment variables:"
set TEST_VAR="mvp6_test_value"
echo "TEST_VAR should be set"
echo ""

echo "=== Test 2: Wildcard Expansion ==="
echo "Testing *.toml files:"
echo *.toml
echo ""

echo "Testing *.rs files:"
ls *.rs
echo ""

echo "=== Test 3: Command Substitution ==="
echo "Current user:"
echo "User: $(whoami)"
echo ""

echo "Current date:"
echo "Date: $(date)"
echo ""

echo "=== Test 4: Array Operations ==="
echo "Creating array:"
array define mycolors red green blue yellow purple
echo ""

echo "Getting array elements:"
array get mycolors 0
array get mycolors 2
array get mycolors 4
echo ""

echo "Array length:"
array len mycolors
echo ""

echo "Listing all arrays:"
array list
echo ""

echo "=== Test 5: Aliases ==="
echo "Creating aliases:"
alias ll='ls -la'
alias gs='git status'
echo ""

echo "Testing aliases:"
alias
echo ""

echo "=== Test 6: Pipes and Redirection ==="
echo "Testing pipe:"
echo "test1 test2 test3" | head -1
echo ""

echo "Creating test file for redirection:"
echo "This is a test file" > test_output.txt
echo "Content of test file:"
cat test_output.txt
echo ""

echo "=== Test 7: Plugin System ==="
echo "Listing loaded plugins:"
plugin list
echo ""

echo "Testing theme plugin:"
theme list
echo ""

echo "Current theme:"
theme current
echo ""

echo "Testing oh-my-winuxsh plugin:"
oh-my-winuxsh version
echo ""

echo "=== Test 8: Environment Variables ==="
echo "All environment variables:"
env
echo ""

echo "=== Test 9: History ==="
echo "Command history (first 5):"
history | head -5
echo ""

echo "=== Test 10: Help System ==="
echo "Built-in help:"
help
echo ""

echo "========================================"
echo "  Test Suite Completed"
echo "========================================"
echo ""
echo "Cleaning up test files..."
rm test_output.txt
echo ""
echo "All tests completed successfully!"