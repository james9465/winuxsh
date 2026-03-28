#!/bin/bash

# Test environment variable setting
set EDITOR=notepad
set VISUAL=vim

echo "EDITOR=$EDITOR"
echo "VISUAL=$VISUAL"

# Test array
array define test a b c
echo "Array test:"
array list