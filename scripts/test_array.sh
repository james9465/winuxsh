#!/bin/bash

# Test array definition
array define fruits apple banana cherry

# Test array listing
echo "All arrays:"
array list

# Test array length
echo "Array length:"
array len fruits

# Test array access
echo "First element:"
array get fruits 0

echo "Second element:"
array get fruits 1

echo "Third element:"
array get fruits 2

echo "Fourth element (out of bounds):"
array get fruits 3