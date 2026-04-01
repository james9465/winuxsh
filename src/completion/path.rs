// Path completion for WinSH
// Provides Tab completion for files and directories

use std::path::{Path, PathBuf};
use std::fs;
use crate::completion::{CompletionContext, CompletionResult};
use crate::error::Result;

/// Path completer
pub struct PathCompleter;

impl PathCompleter {
    /// Complete a path
    pub fn complete(context: &CompletionContext) -> Result<Option<CompletionResult>> {
        let word = match context.get_current_word() {
            Some(w) => w,
            None => return Ok(None),
        };

        // Determine base directory and prefix
        let (base_dir, prefix) = if word.starts_with('/') || word.starts_with('\\') {
            // Absolute path
            (PathBuf::from("/"), word[1..].to_string())
        } else if word.contains('/') || word.contains('\\') {
            // Relative path with directory separator
            let last_sep = word.rfind(|c: char| c == '/' || c == '\\').unwrap();
            let dir_part = &word[..last_sep];
            let prefix_part = &word[last_sep + 1..];
            (context.current_dir.join(dir_part), prefix_part.to_string())
        } else if word.starts_with('.') {
            // Current directory
            (context.current_dir.clone(), word[1..].to_string())
        } else {
            // No path separator, assume current directory
            (context.current_dir.clone(), word.clone())
        };

        // Try to read directory
        let entries = match fs::read_dir(&base_dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(None),
        };

        // Filter and collect matches
        let mut completions: Vec<String> = Vec::new();

        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Check if matches prefix
            if file_name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                let file_type = match entry.file_type() {
                    Ok(ft) => ft,
                    Err(_) => continue,
                };

                // Add separator for directories
                let completion = if file_type.is_dir() {
                    format!("{}/", file_name)
                } else {
                    file_name.clone()
                };

                completions.push(completion);
            }
        }

        // Add ./ prefix if original word started with .
        if word.starts_with('.') {
            completions = completions
                .into_iter()
                .map(|c| format!("./{}", c))
                .collect();
        }

        if completions.is_empty() {
            Ok(None)
        } else {
            completions.sort();
            Ok(Some(CompletionResult::new(completions)))
        }
    }

    /// Expand tilde (~) to home directory
    pub fn expand_tilde(path: &str) -> String {
        if path.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                return path.replacen('~', &home.to_string_lossy(), 1);
            }
        }
        path.to_string()
    }

    /// Get directory completion suggestions
    pub fn get_directories(base_dir: &Path) -> Vec<String> {
        let mut dirs = Vec::new();

        if let Ok(entries) = fs::read_dir(base_dir) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        let dir_name = entry.file_name().to_string_lossy().to_string();
                        dirs.push(format!("{}/", dir_name));
                    }
                }
            }
        }

        dirs.sort();
        dirs
    }

    /// Get file completion suggestions
    pub fn get_files(base_dir: &Path) -> Vec<String> {
        let mut files = Vec::new();

        if let Ok(entries) = fs::read_dir(base_dir) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_file() {
                        let file_name = entry.file_name().to_string_lossy().to_string();
                        files.push(file_name);
                    }
                }
            }
        }

        files.sort();
        files
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        let home = dirs::home_dir().unwrap().to_string_lossy().to_string();
        let expanded = PathCompleter::expand_tilde("~/test");
        assert!(expanded.starts_with(&home));
        assert!(expanded.ends_with("/test"));
    }

    #[test]
    fn test_expand_tilde_no_tilde() {
        let result = PathCompleter::expand_tilde("/absolute/path");
        assert_eq!(result, "/absolute/path");
    }
}