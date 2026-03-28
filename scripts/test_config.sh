#!/bin/bash

# Test that environment variables from config are loaded
echo "EDITOR=$EDITOR"
echo "VISUAL=$VISUAL"

# Test array functionality
array define colors red green blue
echo "Array colors defined"
array list