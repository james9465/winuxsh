// Completion module for WinSH
// Provides Tab completion for commands, paths, and variables

pub mod command;
pub mod completer;
pub mod path;
pub mod variables;

pub use completer::{WinuxshCompleter, CompletionState};

use std::path::PathBuf;

/// Completion context
pub struct CompletionContext {
    /// Current working directory
    pub current_dir: PathBuf,
    /// Current input line
    pub input: String,
    /// Cursor position in input
    pub cursor_pos: usize,
}

impl CompletionContext {
    pub fn new(current_dir: PathBuf, input: String, cursor_pos: usize) -> Self {
        Self {
            current_dir,
            input,
            cursor_pos,
        }
    }

    /// Get the word under cursor
    pub fn get_current_word(&self) -> Option<String> {
        let before_cursor = &self.input[..self.cursor_pos];
        
        // Find the start of the current word
        let word_start = before_cursor
            .rfind(|c: char| c.is_whitespace() || c == ';' || c == '|' || c == '&')
            .map(|pos| pos + 1)
            .unwrap_or(0);

        if word_start < before_cursor.len() {
            Some(before_cursor[word_start..].to_string())
        } else {
            None
        }
    }

    /// Check if cursor is at command position (first word or after separator)
    pub fn is_command_position(&self) -> bool {
        let before_cursor = &self.input[..self.cursor_pos];
        
        // Check if we're at the beginning
        if before_cursor.trim().is_empty() {
            return true;
        }

        // Check if previous character is a command separator
        let last_sep = before_cursor
            .rfind(|c: char| c == ';' || c == '|' || c == '&' || c == '\n');
        
        if let Some(pos) = last_sep {
            // Check if there's only whitespace after the separator
            let after_sep = &before_cursor[pos + 1..];
            after_sep.trim().is_empty()
        } else {
            // No separator found, check if we're at the start
            let trimmed = before_cursor.trim_start();
            trimmed.is_empty()
        }
    }

    /// Check if current word is a path (contains / or \ or starts with .)
    pub fn is_path_completion(&self) -> bool {
        if let Some(word) = self.get_current_word() {
            // Explicit path indicators
            if word.contains('/') || word.contains('\\') || word.starts_with('.') {
                return true;
            }
            
            // If not at command position, treat as path
            if !self.is_command_position() {
                return true;
            }
            
            false
        } else {
            false
        }
    }

    /// Check if current word is a variable (starts with $)
    pub fn is_variable_completion(&self) -> bool {
        if let Some(word) = self.get_current_word() {
            word.starts_with('$')
        } else {
            false
        }
    }
}

/// Completion result
pub struct CompletionResult {
    /// Completions
    pub completions: Vec<String>,
    /// Common prefix (for partial completion)
    pub common_prefix: Option<String>,
}

impl CompletionResult {
    pub fn new(completions: Vec<String>) -> Self {
        let common_prefix = Self::find_common_prefix(&completions);
        Self {
            completions,
            common_prefix,
        }
    }

    fn find_common_prefix(completions: &[String]) -> Option<String> {
        if completions.is_empty() {
            return None;
        }

        let first = &completions[0];
        let mut prefix_len = first.len();

        for completion in completions.iter().skip(1) {
            while !completion.starts_with(&first[..prefix_len]) && prefix_len > 0 {
                prefix_len -= 1;
            }
            if prefix_len == 0 {
                return None;
            }
        }

        Some(first[..prefix_len].to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_current_word() {
        let ctx = CompletionContext::new(
            PathBuf::from("/home/user"),
            "echo hello world".to_string(),
            10,
        );
        assert_eq!(ctx.get_current_word(), Some("hello".to_string()));
    }

    #[test]
    fn test_is_path_completion() {
        let ctx = CompletionContext::new(
            PathBuf::from("/home/user"),
            "cat /tmp/fil".to_string(),
            12,
        );
        assert!(ctx.is_path_completion());
    }

    #[test]
    fn test_is_variable_completion() {
        let ctx = CompletionContext::new(
            PathBuf::from("/home/user"),
            "echo $HOM".to_string(),
            9,
        );
        assert!(ctx.is_variable_completion());
    }
}