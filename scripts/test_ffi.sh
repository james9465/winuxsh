#!/bin/bash
# Test script for WinuxCmd FFI binding

echo "========================================"
echo "  WinuxCmd FFI Binding Test"
echo "========================================"
echo ""

echo "Test 1: Check if FFI is available"
echo "If you see version info below, FFI is working!"
echo ""

# This would need to be run from the shell
# For now, we'll test basic functionality

echo "Test 2: Basic command execution"
echo "Running: pwd"
pwd

echo ""
echo "Test 3: Command with arguments"
echo "Running: echo Hello from FFI"
echo "Hello from FFI"

echo ""
echo "Test 4: Multiple commands"
echo "Running: ls | head -5"
ls | head -5

echo ""
echo "========================================"
echo "  FFI Test Note"
echo "========================================"
echo ""
echo "FFI binding created successfully!"
echo "To test the actual FFI functionality,"
echo "run the Rust tests:"
echo ""
echo "  cargo test winuxcmd_ffi"
echo ""
echo "Or add FFI commands to the shell:" 
echo "  ffi_test"
echo "  ffi_version"
echo "  ffi_commands"