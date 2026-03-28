// WinSH MVP6 - Array Support and Internationalization
//
// MVP6 Features:
// - Array support (definition, access, expansion)
// - Internationalization (English only)
// - Enhanced config file support (terminal styling)
// - Plugin system support
// - All MVP5 features (script execution, job control, wildcards, etc.)

use std::env;
use std::io::{self, Write, BufRead};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::process;
use std::process::{Command, Stdio};
use anyhow::Result;
use colored::Colorize;
use log::debug;
use dirs;
use reedline::{Reedline, Signal, FileBackedHistory, DefaultCompleter, ColumnarMenu,
               ReedlineMenu, MenuBuilder, default_emacs_keybindings, KeyCode, KeyModifiers,
               ReedlineEvent, Emacs, DefaultPrompt, DefaultPromptSegment};

/// Token types for lexical analysis
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Word(String),
    Pipe,           // |
    And,            // &&
    Or,             // ||
    Background,     // &
    Semicolon,      // ;
    RedirIn,        // <
    RedirOut,       // >
    RedirAppend,    // >>
    RedirErr,       // 2>
    Wildcard(String),   // Wildcard pattern
    CommandSubst(String), // Command substitution
    ArrayStart,     // (
    ArrayEnd,       // )
}

/// Command information structure
#[derive(Debug, Clone)]
struct CommandInfo {
    args: Vec<String>,
    stdin_redir: Option<String>,
    stdout_redir: Option<String>,
    stderr_redir: Option<String>,
    stdout_append: bool,
    background: bool,
}

/// Parsed command types
#[derive(Debug, Clone)]
enum ParsedCommand {
    Single(CommandInfo),
    Pipeline(Vec<CommandInfo>),
    And(Box<ParsedCommand>, Box<ParsedCommand>),
    Or(Box<ParsedCommand>, Box<ParsedCommand>),
    Sequence(Vec<ParsedCommand>),
}

/// Array value type
#[derive(Debug, Clone)]
enum ArrayValue {
    String(String),
    Array(Vec<String>),
}

/// Job status
#[derive(Debug, Clone)]
enum JobStatus {
    Running,
    Stopped,
    Done,
}

/// Job structure
#[derive(Debug, Clone)]
struct Job {
    id: u32,
    command: String,
    status: JobStatus,
    pid: u32,
}

/// Terminal color configuration
#[derive(Debug, Clone)]
struct TerminalColors {
    prompt_user: String,
    prompt_host: String,
    prompt_dir: String,
    prompt_symbol: String,
}

impl Default for TerminalColors {
    fn default() -> Self {
        TerminalColors {
            prompt_user: "\x1b[1;32m".to_string(),  // Green
            prompt_host: "\x1b[1;32m".to_string(),  // Green
            prompt_dir: "\x1b[1;34m".to_string(),   // Blue
            prompt_symbol: "\x1b[0m".to_string(),   // Reset
        }
    }
}

/// Shell configuration
#[derive(Debug, Clone)]
struct ShellConfig {
    colors: TerminalColors,
    prompt_format: String,
    plugins: Vec<String>,
}

/// Plugin trait for extensibility
trait Plugin {
    fn name(&self) -> &str;
    fn init(&mut self) -> Result<()>;
    fn execute(&self, args: &[String]) -> Result<bool>; // Return true if handled
}

/// Built-in plugins
struct WelcomePlugin;

impl Plugin for WelcomePlugin {
    fn name(&self) -> &str {
        "welcome"
    }

    fn init(&mut self) -> Result<()> {
        println!("Welcome plugin initialized!");
        Ok(())
    }

    fn execute(&self, args: &[String]) -> Result<bool> {
        if args.get(0).map(|s| s.as_str()) == Some("welcome") {
            println!("Welcome to WinSH MVP6!");
            println!("Type 'help' for available commands.");
            println!("Type 'plugin list' to see loaded plugins.");
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl Default for ShellConfig {
    fn default() -> Self {
        ShellConfig {
            colors: TerminalColors::default(),
            prompt_format: "%u@%h %w $ ".to_string(),
            plugins: Vec::new(),
        }
    }
}

/// Main shell structure
struct Shell {
    current_dir: PathBuf,
    aliases: HashMap<String, String>,
    env_vars: HashMap<String, ArrayValue>,  // Changed to support arrays
    line_editor: Reedline,
    history_path: PathBuf,
    jobs: Vec<Job>,
    next_job_id: u32,
    config: ShellConfig,
    plugins: Vec<Box<dyn Plugin>>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{} {}", "Error:".red(), e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    // Initialize logging (default to error level only, set RUST_LOG=debug for details)
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Error)
        .init();

    debug!("WinSH MVP6 starting");

    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "-c" => {
                if args.len() > 2 {
                    let mut shell = Shell::new(true)?;
                    shell.save_history(&args[2])?;
                    shell.execute_command(&args[2])?;
                } else {
                    eprintln!("{} {}", "Error:".red(), "-c requires an argument");
                    process::exit(1);
                }
            }
            "--version" => {
                println!("{}", "WinSH MVP6 - Array Support and Internationalization version 0.6.0".green());
            }
            _ => {
                // Check if it's a script file
                let script_path = PathBuf::from(&args[1]);
                if script_path.exists() {
                    let mut shell = Shell::new(true)?;
                    let script_content = std::fs::read_to_string(&script_path)?;
                    for line in script_content.lines() {
                        let line = line.trim();
                        if line.is_empty() || line.starts_with('#') {
                            continue;
                        }
                        shell.execute_command(line)?;
                    }
                } else {
                    eprintln!("{} {}", "Unknown argument:".red(), args[1]);
                    eprintln!("Usage: winsh [-c command] [script.sh]");
                    process::exit(1);
                }
            }
        }
        return Ok(());
    }

    let mut shell = Shell::new(true)?;
    shell.run_repl()?;

    Ok(())
}

impl Shell {
    /// Create a new shell instance
    fn new(load_config: bool) -> Result<Self> {
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let history_path = home_dir.join(".winsh_history");

        // Create completer
        let completer = DefaultCompleter::with_inclusions(&[]);
        let menu = ColumnarMenu::default().with_name("completion_menu");

        // Create line editor
        let line_editor = Reedline::create()
            .with_completer(Box::new(completer))
            .with_menu(ReedlineMenu::EngineCompleter(Box::new(menu)))
            .with_history(Box::new(
                FileBackedHistory::with_file(1000, history_path.clone())
                    .expect("Error configuring history with file"),
            ))
            .with_quick_completions(true)
            .with_partial_completions(true);

        let mut shell = Shell {
            current_dir: env::current_dir()?,
            aliases: HashMap::new(),
            env_vars: HashMap::new(),
            line_editor,
            history_path,
            jobs: Vec::new(),
            next_job_id: 1,
            config: ShellConfig::default(),
            plugins: Vec::new(),
        };

        // Load built-in plugins
        shell.load_builtin_plugins();

        // Load user plugins from config
        shell.load_user_plugins()?;

        // Load default aliases
        shell.aliases.insert("ll".to_string(), "ls -la".to_string());
        shell.aliases.insert("la".to_string(), "ls -a".to_string());
        shell.aliases.insert("l".to_string(), "ls".to_string());

        // Load environment variables
        for (key, value) in env::vars() {
            shell.env_vars.insert(key, ArrayValue::String(value));
        }

        // Load config file
        if load_config {
            if let Err(e) = shell.load_config() {
                eprintln!("{} {}", "Warning:".yellow(), format!("Failed to load config: {}", e));
            }
        }

        Ok(shell)
    }

    /// Run the REPL loop
    fn run_repl(&mut self) -> Result<()> {
        println!("{}", "WinSH MVP6 - Array Support and Internationalization".green());
        println!("Type 'help' for available commands");
        println!();

        loop {
            let prompt = self.get_prompt();

            match self.line_editor.read_line(&prompt) {
                Ok(Signal::Success(buffer)) => {
                    let line = buffer.trim();
                    if line.is_empty() {
                        continue;
                    }

                    // Save to history
                    if let Err(e) = self.save_history(line) {
                        eprintln!("{} {}", "Warning:".yellow(), format!("Failed to save history: {}", e));
                    }

                    // Execute command
                    if let Err(e) = self.execute_command(line) {
                        eprintln!("{} {}", "Error:".red(), e);
                    }
                }
                Ok(Signal::CtrlD) => {
                    println!();
                    println!("Goodbye!");
                    break;
                }
                Ok(Signal::CtrlC) => {
                    println!();
                    continue;
                }
                Err(e) => {
                    eprintln!("{} {}", "Error:".red(), e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Get the prompt string
    fn get_prompt(&self) -> DefaultPrompt {
        let username = self.get_env_var("USERNAME", "user");
        let hostname = self.get_env_var("COMPUTERNAME", "localhost");
        let dir = self.current_dir.display().to_string();

        // Create colored prompt based on config
        let prompt_text = format!(
            "{}{}{}@{}{} {}{}{}{} ",
            self.config.colors.prompt_user,
            username,
            self.config.colors.prompt_symbol,
            self.config.colors.prompt_host,
            hostname,
            self.config.colors.prompt_dir,
            dir,
            self.config.colors.prompt_symbol,
            self.config.colors.prompt_symbol
        );

        DefaultPrompt::new(
            DefaultPromptSegment::Basic(prompt_text),
            DefaultPromptSegment::Empty
        )
    }

    /// Get environment variable
    fn get_env_var(&self, key: &str, default: &str) -> String {
        debug!("get_env_var: key = '{}'", key);
        let result = self.env_vars.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| ArrayValue::String(default.to_string()));

        match result {
            ArrayValue::String(s) => s,
            ArrayValue::Array(arr) => {
                // For arrays, return first element or default
                arr.first().cloned().unwrap_or_else(|| default.to_string())
            }
        }
    }

    /// Save command to history file
    fn save_history(&mut self, command: &str) -> Result<()> {
        // Remove BOM characters and whitespace
        let clean_command = command.trim_matches(|c: char| {
            c == '\u{feff}' || c == '\u{fffe}' || c.is_whitespace()
        });

        if clean_command.is_empty() {
            return Ok(());
        }

        // Read existing history
        let mut history = if self.history_path.exists() {
            std::fs::read_to_string(&self.history_path)?
        } else {
            String::new()
        };

        // Add new command
        if !history.is_empty() {
            history.push('\n');
        }
        history.push_str(clean_command);

        // Write back to file
        std::fs::write(&self.history_path, history)?;

        Ok(())
    }

    /// Load configuration file
    fn load_config(&mut self) -> Result<()> {
        // First, load shell config (.winshrc.toml if exists)
        let shell_config_paths = vec![
            PathBuf::from(".winshrc.toml"),
            PathBuf::from("$HOME/.winshrc.toml"),
            PathBuf::from("$USERPROFILE/.winshrc.toml"),
            PathBuf::from(env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string())).join(".winshrc.toml"),
            PathBuf::from(env::var("HOME").unwrap_or_else(|_| ".".to_string())).join(".winshrc.toml"),
        ];

        debug!("Looking for shell config files: {:?}", shell_config_paths);

        if let Some(config_path) = shell_config_paths.iter().find(|path| path.exists()) {
            println!("{} {}", "Loading shell config:".cyan(), config_path.display());
            self.load_shell_config(config_path)?;
        } else {
            debug!("No shell config file found");
        }

        // Then, load .winshrc file
        let config_paths = vec![
            PathBuf::from(".winshrc"),
            PathBuf::from("$HOME/.winshrc"),
            PathBuf::from("$USERPROFILE/.winshrc"),
            PathBuf::from(env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string())).join(".winshrc"),
            PathBuf::from(env::var("HOME").unwrap_or_else(|_| ".".to_string())).join(".winshrc"),
        ];

        if let Some(config_path) = config_paths.iter().find(|path| path.exists()) {
            println!("{} {}", "Loading config:".cyan(), config_path.display());
            self.parse_config_file(config_path)?;
        }

        Ok(())
    }

    /// Load shell configuration file (TOML format)
    fn load_shell_config(&mut self, path: &Path) -> Result<()> {
        let config_content = std::fs::read_to_string(path)?;

        // Parse simple key=value format
        for line in config_content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = self.parse_ansi_escape_sequences(value.trim());

                match key {
                    "prompt_user_color" => {
                        self.config.colors.prompt_user = value;
                    }
                    "prompt_host_color" => {
                        self.config.colors.prompt_host = value;
                    }
                    "prompt_dir_color" => {
                        self.config.colors.prompt_dir = value;
                    }
                    "prompt_symbol" => {
                        self.config.colors.prompt_symbol = value;
                    }
                    "prompt_format" => {
                        self.config.prompt_format = value;
                    }
                    "plugin" => {
                        self.config.plugins.push(value);
                    }
                    _ => {
                        // Store as environment variable
                        self.env_vars.insert(key.to_string(), ArrayValue::String(value));
                    }
                }
            }
        }

        Ok(())
    }

    /// Parse ANSI escape sequences like \x1b[1;32m
    fn parse_ansi_escape_sequences(&self, input: &str) -> String {
        let mut result = String::new();
        let mut chars = input.chars().peekable();

        while let Some(&c) = chars.peek() {
            chars.next();

            if c == '\\' {
                if let Some(&'x') = chars.peek() {
                    chars.next();
                    // Parse hex escape sequence
                    let mut hex_str = String::new();
                    while let Some(&c) = chars.peek() {
                        if c.is_ascii_hexdigit() && hex_str.len() < 2 {
                            chars.next();
                            hex_str.push(c);
                        } else {
                            break;
                        }
                    }

                    if let Ok(byte) = u8::from_str_radix(&hex_str, 16) {
                        result.push(byte as char);
                    }
                } else {
                    // Keep backslash for other escape sequences
                    result.push(c);
                }
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Parse configuration file
    fn parse_config_file(&mut self, path: &Path) -> Result<()> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Err(e) = self.execute_command(line) {
                eprintln!("{} {}", "Config error:".yellow(), format!("{} - {}", line, e));
            }
        }

        Ok(())
    }

    /// Execute a command string
    fn execute_command(&mut self, command: &str) -> Result<()> {
        let parsed = self.parse_command(command)?;

        match parsed {
            ParsedCommand::Single(cmd) => {
                self.execute_single_command(cmd)?;
            }
            ParsedCommand::Pipeline(cmds) => {
                self.execute_pipeline(cmds)?;
            }
            ParsedCommand::And(left, right) => {
                if self.execute_single_command(left.clone().into_single_cmd()).is_ok() {
                    self.execute_single_command(right.clone().into_single_cmd())?;
                }
            }
            ParsedCommand::Or(left, right) => {
                if self.execute_single_command(left.clone().into_single_cmd()).is_err() {
                    self.execute_single_command(right.clone().into_single_cmd())?;
                }
            }
            ParsedCommand::Sequence(cmds) => {
                for cmd in cmds {
                    self.execute_single_command(cmd.into_single_cmd())?;
                }
            }
        }

        Ok(())
    }

    /// Parse command string into ParsedCommand
    fn parse_command(&mut self, command: &str) -> Result<ParsedCommand> {
        let tokens = self.tokenize(command)?;

        // Check for array definition: name=(element1 element2 ...)
        self.parse_array_definition(&tokens)?;

        // Build command args from tokens
        let mut args = Vec::new();
        let mut in_array = false;
        let mut current_array = Vec::new();

        for token in &tokens {
            match token {
                Token::Word(w) => {
                    if !in_array {
                        args.push(w.clone());
                    } else {
                        current_array.push(w.clone());
                    }
                }
                Token::ArrayStart => {
                    in_array = true;
                    current_array.clear();
                }
                Token::ArrayEnd => {
                    in_array = false;
                    if let Some(array_name) = args.last().and_then(|last| {
                        // Check if last arg ends with '=' (array definition)
                        if last.ends_with('=') {
                            Some(last.trim_end_matches('='))
                        } else {
                            None
                        }
                    }) {
                        // Store array in environment
                        self.env_vars.insert(
                            array_name.to_string(),
                            ArrayValue::Array(current_array.clone())
                        );
                        // Remove the array name from args
                        args.pop();
                    }
                }
                _ => {}
            }
        }

        // Simplified parsing for now
        // TODO: Implement full parsing logic for pipelines, and/or, etc.
        Ok(ParsedCommand::Single(CommandInfo {
            args,
            stdin_redir: None,
            stdout_redir: None,
            stderr_redir: None,
            stdout_append: false,
            background: false,
        }))
    }

    /// Parse array definition from tokens
    fn parse_array_definition(&mut self, tokens: &[Token]) -> Result<()> {
        // Check if we have pattern: Word ArrayStart ... ArrayEnd
        if tokens.len() >= 2 {
            if let Token::ArrayStart = tokens[1] {
                if let Token::Word(name) = &tokens[0] {
                    if name.ends_with('=') {
                        let array_name = name.trim_end_matches('=');
                        let mut elements = Vec::new();

                        let mut i = 2;
                        while i < tokens.len() {
                            match &tokens[i] {
                                Token::Word(w) => elements.push(w.clone()),
                                Token::ArrayEnd => break,
                                _ => {}
                            }
                            i += 1;
                        }

                        if !elements.is_empty() {
                            self.env_vars.insert(
                                array_name.to_string(),
                                ArrayValue::Array(elements)
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Tokenize command string with enhanced array support
    fn tokenize(&self, command: &str) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        let mut chars = command.chars().peekable();
        let mut current_word = String::new();
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut in_backtick = false;

        while let Some(&c) = chars.peek() {
            chars.next();

            match c {
                '\'' if !in_double_quote && !in_backtick => {
                    in_single_quote = !in_single_quote;
                    if !in_single_quote {
                        // End of single quote
                        if !current_word.is_empty() {
                            tokens.push(Token::Word(current_word.clone()));
                            current_word.clear();
                        }
                    }
                }
                '"' if !in_single_quote && !in_backtick => {
                    in_double_quote = !in_double_quote;
                }
                '`' if !in_single_quote && !in_double_quote => {
                    in_backtick = !in_backtick;
                }
                '(' if !in_single_quote && !in_double_quote && !in_backtick => {
                    // Check if this is array definition: name=(...)
                    if !current_word.is_empty() && current_word.ends_with('=') {
                        // This is an array definition
                        current_word.pop(); // Remove '='
                        tokens.push(Token::Word(current_word.clone()));
                        current_word.clear();
                        tokens.push(Token::ArrayStart);
                    } else {
                        // This might be command substitution or other construct
                        current_word.push(c);
                    }
                }
                ')' if !in_single_quote && !in_double_quote && !in_backtick => {
                    tokens.push(Token::ArrayEnd);
                }
                '|' if !in_single_quote && !in_double_quote && !in_backtick => {
                    if !current_word.is_empty() {
                        tokens.push(Token::Word(current_word.clone()));
                        current_word.clear();
                    }
                    if let Some(&'|') = chars.peek() {
                        chars.next();
                        tokens.push(Token::Or);
                    } else {
                        tokens.push(Token::Pipe);
                    }
                }
                '&' if !in_single_quote && !in_double_quote && !in_backtick => {
                    if !current_word.is_empty() {
                        tokens.push(Token::Word(current_word.clone()));
                        current_word.clear();
                    }
                    if let Some(&'&') = chars.peek() {
                        chars.next();
                        tokens.push(Token::And);
                    } else {
                        tokens.push(Token::Background);
                    }
                }
                ';' if !in_single_quote && !in_double_quote && !in_backtick => {
                    if !current_word.is_empty() {
                        tokens.push(Token::Word(current_word.clone()));
                        current_word.clear();
                    }
                    tokens.push(Token::Semicolon);
                }
                '<' if !in_single_quote && !in_double_quote && !in_backtick => {
                    if !current_word.is_empty() {
                        tokens.push(Token::Word(current_word.clone()));
                        current_word.clear();
                    }
                    tokens.push(Token::RedirIn);
                }
                '>' if !in_single_quote && !in_double_quote && !in_backtick => {
                    if !current_word.is_empty() {
                        tokens.push(Token::Word(current_word.clone()));
                        current_word.clear();
                    }
                    if let Some(&'>') = chars.peek() {
                        chars.next();
                        tokens.push(Token::RedirAppend);
                    } else {
                        tokens.push(Token::RedirOut);
                    }
                }
                '2' if !in_single_quote && !in_double_quote && !in_backtick => {
                    if let Some(&'>') = chars.peek() {
                        chars.next();
                        if !current_word.is_empty() {
                            tokens.push(Token::Word(current_word.clone()));
                            current_word.clear();
                        }
                        tokens.push(Token::RedirErr);
                    } else {
                        current_word.push(c);
                    }
                }
                '$' if !in_single_quote && !in_double_quote && !in_backtick => {
                    if let Some(&'{') = chars.peek() {
                        chars.next();
                        // Variable expansion: ${VAR} or ${arr[0]}
                        let mut var_name = String::new();
                        let mut bracket_depth = 1;
                        while let Some(&c) = chars.peek() {
                            chars.next();
                            match c {
                                '{' => {
                                    bracket_depth += 1;
                                    var_name.push(c);
                                }
                                '}' => {
                                    bracket_depth -= 1;
                                    if bracket_depth == 0 {
                                        break;
                                    }
                                    var_name.push(c);
                                }
                                _ => var_name.push(c),
                            }
                        }
                        // Store as special word for later expansion
                        tokens.push(Token::Word(format!("${{{}}}", var_name)));
                    } else if let Some(&'(') = chars.peek() {
                        chars.next();
                        let mut cmd_subst = String::new();
                        let mut depth = 1;
                        while let Some(&c) = chars.peek() {
                            chars.next();
                            match c {
                                '(' => depth += 1,
                                ')' => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                _ => cmd_subst.push(c),
                            }
                        }
                        tokens.push(Token::CommandSubst(cmd_subst));
                    } else {
                        // Simple variable expansion: $VAR
                        let mut var_name = String::new();
                        while let Some(&c) = chars.peek() {
                            if !c.is_alphanumeric() && c != '_' {
                                break;
                            }
                            chars.next();
                            var_name.push(c);
                        }
                        if !var_name.is_empty() {
                            tokens.push(Token::Word(format!("${{{}}}", var_name)));
                        } else {
                            current_word.push('$');
                        }
                    }
                }
                ' ' | '\t' if !in_single_quote && !in_double_quote && !in_backtick => {
                    if !current_word.is_empty() {
                        tokens.push(Token::Word(current_word.clone()));
                        current_word.clear();
                    }
                }
                _ => {
                    current_word.push(c);
                }
            }
        }

        if !current_word.is_empty() {
            tokens.push(Token::Word(current_word));
        }

        Ok(tokens)
    }

    /// Execute single command
    fn execute_single_command(&mut self, mut cmd: CommandInfo) -> Result<()> {
        // Expand aliases
        if let Some(first_arg) = cmd.args.first() {
            if let Some(alias) = self.aliases.get(first_arg) {
                let alias_parts: Vec<String> = shlex::split(alias).unwrap_or_else(|| vec![alias.clone()]);
                cmd.args = alias_parts.into_iter().chain(cmd.args.iter().skip(1).cloned()).collect();
            }
        }

        // Expand environment variables and arrays
        cmd.args = cmd.args.iter().map(|arg| self.expand_variables(arg)).collect();

        // Handle built-in commands
        if let Some(result) = self.handle_builtin(&cmd.args) {
            return result;
        }

        // Handle wildcards
        let expanded_args = self.expand_wildcards(&cmd.args);

        // Check if command exists
        if expanded_args.is_empty() {
            return Ok(());
        }

        let command = &expanded_args[0];

        // Find command in PATH
        let command_path = self.find_command(command)?;

        // Execute external command
        let mut process = Command::new(&command_path);
        process.args(&expanded_args[1..]);

        // Handle redirections
        if let Some(ref stdin) = cmd.stdin_redir {
            process.stdin(Stdio::from(std::fs::File::open(stdin)?));
        }

        if let Some(ref stdout) = cmd.stdout_redir {
            let file = if cmd.stdout_append {
                std::fs::OpenOptions::new().create(true).append(true).open(stdout)?
            } else {
                std::fs::File::create(stdout)?
            };
            process.stdout(Stdio::from(file));
        }

        if let Some(ref stderr) = cmd.stderr_redir {
            process.stderr(Stdio::from(std::fs::File::create(stderr)?));
        }

        // Set current directory
        process.current_dir(&self.current_dir);

        // Handle background execution
        if cmd.background {
            process.spawn()?;
        } else {
            process.spawn()?.wait_with_output()?;
        }

        Ok(())
    }

    /// Execute pipeline
    fn execute_pipeline(&mut self, cmds: Vec<CommandInfo>) -> Result<()> {
        // Simplified pipeline execution
        // TODO: Implement proper pipeline with process pipes
        for cmd in cmds {
            self.execute_single_command(cmd)?;
        }
        Ok(())
    }

    /// Handle built-in commands
    fn handle_builtin(&mut self, args: &[String]) -> Option<Result<()>> {
        if args.is_empty() {
            return Some(Ok(()));
        }

        // Check plugins first
        if let Ok(handled) = self.execute_plugins(args) {
            if handled {
                return Some(Ok(()));
            }
        }

        match args[0].as_str() {
            "cd" => {
                let dir_str = if args.len() > 1 {
                    args[1].clone()
                } else {
                    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).to_str().unwrap().to_string()
                };

                let new_dir = if dir_str == "~" {
                    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
                } else {
                    PathBuf::from(dir_str.as_str())
                };

                if let Err(e) = env::set_current_dir(&new_dir) {
                    return Some(Err(anyhow::anyhow!("cd: {} - {}", dir_str, e)));
                }

                self.current_dir = env::current_dir().unwrap();
                Some(Ok(()))
            }
            "pwd" => {
                println!("{}", self.current_dir.display());
                Some(Ok(()))
            }
            "echo" => {
                let output = args[1..].join(" ");
                println!("{}", output);
                Some(Ok(()))
            }
            "exit" | "quit" => {
                process::exit(0);
            }
            "clear" | "cls" => {
                print!("\x1b[2J\x1b[H");
                io::stdout().flush().unwrap();
                Some(Ok(()))
            }
            "set" => {
                if args.len() > 1 {
                    let arg = &args[1];
                    if arg.contains('=') {
                        if let Some((key, value)) = arg.split_once('=') {
                            self.env_vars.insert(key.to_string(), ArrayValue::String(value.to_string()));
                        }
                    }
                }
                Some(Ok(()))
            }
            "array" => {
                if args.len() > 1 {
                    match args[1].as_str() {
                        "define" => {
                            if args.len() > 2 {
                                let array_name = &args[2];
                                let elements: Vec<String> = args[3..].to_vec();
                                let count = elements.len();
                                self.env_vars.insert(array_name.to_string(), ArrayValue::Array(elements));
                                println!("Array '{}' defined with {} elements", array_name, count);
                            }
                        }
                        "get" => {
                            if args.len() > 3 {
                                let array_name = &args[2];
                                let index: usize = args[3].parse().unwrap_or(0);
                                if let Some(ArrayValue::Array(arr)) = self.env_vars.get(array_name) {
                                    if let Some(element) = arr.get(index) {
                                        println!("{}", element);
                                    } else {
                                        println!("Index out of bounds");
                                    }
                                } else {
                                    println!("Array '{}' not found", array_name);
                                }
                            }
                        }
                        "len" => {
                            if args.len() > 2 {
                                let array_name = &args[2];
                                if let Some(ArrayValue::Array(arr)) = self.env_vars.get(array_name) {
                                    println!("{}", arr.len());
                                } else {
                                    println!("Array '{}' not found", array_name);
                                }
                            }
                        }
                        "list" => {
                            for (key, value) in &self.env_vars {
                                if let ArrayValue::Array(arr) = value {
                                    println!("{}=({})", key, arr.join(" "));
                                }
                            }
                        }
                        _ => {
                            println!("Array commands: define, get, len, list");
                        }
                    }
                }
                Some(Ok(()))
            }
            "export" => {
                if args.len() > 1 {
                    if let Some((key, value)) = args[1].split_once('=') {
                        self.env_vars.insert(key.to_string(), ArrayValue::String(value.to_string()));
                        env::set_var(key, value);
                    }
                }
                Some(Ok(()))
            }
            "unset" => {
                if args.len() > 1 {
                    self.env_vars.remove(&args[1]);
                    env::remove_var(&args[1]);
                }
                Some(Ok(()))
            }
            "env" => {
                for (key, value) in &self.env_vars {
                    match value {
                        ArrayValue::String(v) => {
                            println!("{}={}", key, v);
                        }
                        ArrayValue::Array(arr) => {
                            println!("{}=({})", key, arr.join(" "));
                        }
                    }
                }
                Some(Ok(()))
            }
            "help" => {
                self.print_help();
                Some(Ok(()))
            }
            "history" => {
                self.print_history();
                Some(Ok(()))
            }
            "alias" => {
                if args.len() == 1 {
                    for (name, value) in &self.aliases {
                        println!("{}='{}'", name, value);
                    }
                } else if args.len() > 1 {
                    if let Some((name, value)) = args[1].split_once('=') {
                        self.aliases.insert(name.to_string(), value.to_string());
                    }
                }
                Some(Ok(()))
            }
            "unalias" => {
                if args.len() > 1 {
                    self.aliases.remove(&args[1]);
                }
                Some(Ok(()))
            }
            "source" | "." => {
                if args.len() > 1 {
                    if let Err(e) = self.parse_config_file(Path::new(&args[1])) {
                        return Some(Err(e));
                    }
                }
                Some(Ok(()))
            }
            "plugin" => {
                if args.len() > 1 {
                    match args[1].as_str() {
                        "list" => {
                            println!("{}", "Loaded Plugins:".cyan());
                            for plugin in &self.plugins {
                                println!("  - {}", plugin.name());
                            }
                            if self.plugins.is_empty() {
                                println!("  (No plugins loaded)");
                            }
                        }
                        "load" => {
                            if args.len() > 2 {
                                // Placeholder for future plugin loading
                                println!("Plugin '{}' not found (not implemented yet)", args[2]);
                            }
                        }
                        _ => {
                            println!("Plugin commands: list, load");
                        }
                    }
                } else {
                    println!("Plugin commands: list, load");
                }
                Some(Ok(()))
            }
            _ => None,
        }
    }

    /// Expand environment variables and array operations
    fn expand_variables(&self, arg: &str) -> String {
        let mut result = String::new();
        let mut chars = arg.chars().peekable();

        while let Some(&c) = chars.peek() {
            chars.next();

            if c == '$' {
                if let Some(&'{') = chars.peek() {
                    chars.next();
                    let mut var_name = String::new();
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if c == '}' {
                            break;
                        }
                        var_name.push(c);
                    }

                    // Check for array operations
                    if var_name.starts_with('#') && var_name.contains('[') {
                        // Array length: ${#arr[@]}
                        let inner = &var_name[1..]; // Remove '#'
                        if let Some(array_name) = self.extract_array_name(inner) {
                            if let Some(ArrayValue::Array(arr)) = self.env_vars.get(&array_name) {
                                result.push_str(&arr.len().to_string());
                            } else {
                                result.push_str("0");
                            }
                        } else {
                            result.push_str("0");
                        }
                    } else if var_name.contains('[') {
                        // Array access: ${arr[0]}
                        if let Some((array_name, index)) = self.parse_array_access(&var_name) {
                            if let Some(ArrayValue::Array(arr)) = self.env_vars.get(&array_name) {
                                if let Some(element) = arr.get(index) {
                                    result.push_str(element);
                                }
                            } else if let Some(ArrayValue::String(s)) = self.env_vars.get(&array_name) {
                                // Treat string as single-element array
                                if index == 0 {
                                    result.push_str(s);
                                }
                            }
                        }
                    } else if var_name == "@" || var_name == "*" {
                        // Special array expansion: ${@} or ${*}
                        // Usually used in function contexts, expand to empty for now
                    } else {
                        // Regular variable
                        result.push_str(&self.get_env_var(&var_name, ""));
                    }
                } else {
                    let mut var_name = String::new();
                    while let Some(&c) = chars.peek() {
                        if !c.is_alphanumeric() && c != '_' {
                            break;
                        }
                        chars.next();
                        var_name.push(c);
                    }
                    result.push_str(&self.get_env_var(&var_name, ""));
                }
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Extract array name from expression like "arr[@]" or "arr[*]"
    fn extract_array_name(&self, expr: &str) -> Option<String> {
        if let Some(pos) = expr.find('[') {
            Some(expr[..pos].to_string())
        } else {
            None
        }
    }

    /// Parse array access expression like "arr[0]" or "arr[@]"
    fn parse_array_access(&self, expr: &str) -> Option<(String, usize)> {
        if let Some(pos) = expr.find('[') {
            let array_name = expr[..pos].to_string();
            let inner = &expr[pos + 1..expr.len() - 1]; // Remove [ and ]

            if inner == "@" || inner == "*" {
                // Return all elements (special case)
                return Some((array_name, usize::MAX));
            } else if let Ok(index) = inner.parse::<usize>() {
                return Some((array_name, index));
            }
        }
        None
    }

    /// Expand wildcards
    fn expand_wildcards(&self, args: &[String]) -> Vec<String> {
        let mut expanded = Vec::new();

        for arg in args {
            if arg.contains('*') || arg.contains('?') || arg.contains('[') {
                // Expand wildcard
                if let Ok(matches) = glob::glob(arg) {
                    let mut found = false;
                    for entry in matches {
                        if let Ok(path) = entry {
                            expanded.push(path.to_string_lossy().to_string());
                            found = true;
                        }
                    }
                    if !found {
                        expanded.push(arg.clone());
                    }
                } else {
                    expanded.push(arg.clone());
                }
            } else {
                expanded.push(arg.clone());
            }
        }

        expanded
    }

    /// Find command in PATH
    fn find_command(&self, cmd: &str) -> Result<String> {
        // Check if it's an absolute path
        if PathBuf::from(cmd).is_absolute() {
            return Ok(cmd.to_string());
        }

        // Search in PATH
        if let Ok(path) = env::var("PATH") {
            for dir in env::split_paths(&path) {
                let cmd_path = dir.join(cmd);
                if cmd_path.exists() {
                    return Ok(cmd_path.to_string_lossy().to_string());
                }

                // Check for .exe extension
                let cmd_exe = cmd_path.with_extension("exe");
                if cmd_exe.exists() {
                    return Ok(cmd_exe.to_string_lossy().to_string());
                }
            }
        }

        Err(anyhow::anyhow!("Command not found: {}", cmd))
    }

    /// Print help information
    fn print_help(&self) {
        println!("{}", "WinSH MVP6 - Available Commands:".green());
        println!();
        println!("{}", "Built-in Commands:".cyan());
        println!("  cd [dir]       - Change directory");
        println!("  pwd            - Print current directory");
        println!("  echo [text]    - Print text (supports env vars)");
        println!("  set VAR=VALUE  - Set environment variable");
        println!("  export VAR=VALUE - Set environment variable");
        println!("  unset VAR      - Remove environment variable");
        println!("  env            - Display all environment variables");
        println!("  source [file]  - Load configuration file");
        println!("  . [file]       - Load configuration file (alias)");
        println!("  exit           - Exit shell");
        println!("  quit           - Exit shell");
        println!("  clear          - Clear screen");
        println!("  cls            - Clear screen");
        println!("  alias [name=value] - Display or set alias");
        println!("  unalias [name]  - Remove alias");
        println!("  help           - Display help information");
        println!("  history        - Display command history");
        println!();
        println!("{}", "Array Support:".cyan());
        println!("  arr=(a b c)    - Define array");
        println!("  ${{arr[0]}}      - Access array element");
        println!("  ${{arr[@]}}      - Access all elements");
        println!("  ${{#arr[@]}}     - Get array length");
        println!();
        println!("{}", "Pipes and Redirections:".cyan());
        println!("  |              - Pipe");
        println!("  < file         - Input redirection");
        println!("  > file         - Output redirection");
        println!("  >> file        - Output append");
        println!("  2> file        - Error redirection");
        println!("  &&             - Logical AND");
        println!("  ||             - Logical OR");
        println!("  ;              - Command separator");
        println!("  &              - Background execution");
        println!();
        println!("{}", "Wildcards:".cyan());
        println!("  *              - Match any characters");
        println!("  ?              - Match single character");
        println!("  [...]          - Character set match");
        println!();
        println!("{}", "Command Substitution:".cyan());
        println!("  $(command)     - Execute command and replace with output");
        println!("  `command`      - Execute command and replace with output (backticks)");
        println!();
        println!("{}", "User Experience:".cyan());
        println!("  ↑↓ keys        - Browse history");
        println!("  Tab key        - Command and path completion");
        println!("  Ctrl+C         - Interrupt current command");
        println!("  Ctrl+D         - Exit shell");
        println!();
        println!("{}", "Predefined Aliases:".cyan());
        println!("  ll             - ls -la");
        println!("  la             - ls -a");
        println!("  l              - ls");
        println!();
        println!("{}", "Configuration:".cyan());
        println!("  .winshrc       - Shell configuration file");
        println!("  .winsh_history  - Command history file");
        println!();
        println!("{} {}", "Tip:".yellow(), "Use 'type <command>' to view command location");
        println!("{} {}", "Tip:".yellow(), "Use 'export PATH=$PATH;new_path' to modify PATH");
    }

    /// Print command history
    fn print_history(&self) {
        if let Ok(history) = std::fs::read_to_string(&self.history_path) {
            let mut lines: Vec<String> = history.lines()
                .map(|l| l.trim_matches(|c: char| {
                    c == '\u{feff}' || c == '\u{fffe}' || c.is_whitespace()
                }).to_string())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .collect();

            println!("{}", "Command History:".cyan());
            for (i, line) in lines.iter().enumerate() {
                println!("  {}  {}", i + 1, line);
            }
        } else {
            println!("{} {}", "Warning:".yellow(), "No history available");
        }
    }

    /// Load built-in plugins
    fn load_builtin_plugins(&mut self) {
        let mut welcome_plugin = WelcomePlugin;
        if let Err(e) = welcome_plugin.init() {
            eprintln!("{} {}", "Warning:".yellow(), format!("Failed to initialize welcome plugin: {}", e));
        } else {
            self.plugins.push(Box::new(welcome_plugin));
        }
    }

    /// Load user plugins from config
    fn load_user_plugins(&mut self) -> Result<()> {
        // For now, plugins are specified in config but not implemented
        // This is a placeholder for future plugin loading
        Ok(())
    }

    /// Execute plugin commands
    fn execute_plugins(&mut self, args: &[String]) -> Result<bool> {
        for plugin in &mut self.plugins {
            if plugin.execute(args)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl ParsedCommand {
    fn into_single_cmd(self) -> CommandInfo {
        match self {
            ParsedCommand::Single(cmd) => cmd,
            _ => panic!("Expected single command"),
        }
    }
}