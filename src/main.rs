// WinSH MVP6 - Array Support and Internationalization
//
// MVP6 Features:
// - Array support (definition, access, expansion)
// - Internationalization (English only)
// - Enhanced config file support (terminal styling)
// - Plugin system support
// - Modular architecture following Rust best practices

use anyhow::Result;
use colored::Colorize;
use reedline::Signal;
use std::env;
use std::path::PathBuf;

mod array;
mod builtins;
mod config;
mod error;
mod executor;
mod job;
mod oh_my_winuxsh;
mod parser;
mod plugin;
mod shell;
mod theme;
mod tokenizer;
mod winuxcmd_ffi;

use shell::Shell;
use winuxcmd_ffi::WinuxCmdFFI;

fn print_usage() {
    println!("WinSH usage:");
    println!("  winuxsh -c \"command\"");
    println!("  winuxsh script.sh [args...]");
    println!("  winuxsh --help | -h");
    println!("  winuxsh --version");
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{} {}", "Error:".red(), e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    // Initialize logging (default to error level only)
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Error)
        .init();

    // Initialize WinuxCmd daemon
    if let Err(e) = initialize_winuxcmd_daemon() {
        eprintln!("{} {}", "Warning:".yellow(), format!("Failed to initialize WinuxCmd daemon: {}", e));
    }

    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "-c" => {
                if args.len() > 2 {
                    let mut shell = Shell::new(true)?;
                    if let Err(e) = shell.save_history(&args[2]) {
                        eprintln!(
                            "{} {}",
                            "Warning:".yellow(),
                            format!("Failed to save history: {}", e)
                        );
                    }
                    shell.execute_command(&args[2])?;
                } else {
                    eprintln!("{} {}", "Error:".red(), "-c requires an argument");
                    std::process::exit(1);
                }
            }
            "--help" | "-h" => {
                print_usage();
            }
            "--version" => {
                println!(
                    "{}",
                    "WinSH MVP6 - Array Support and Internationalization version 0.6.0".green()
                );
            }
            _ => {
                // Check if it's a script file
                let script_path = PathBuf::from(&args[1]);
                if script_path.exists() {
                    let mut shell = Shell::new(true)?;
                    shell.run_script_file(&script_path, &args[2..])?;
                } else {
                    eprintln!("{} {}", "Unknown argument:".red(), args[1]);
                    print_usage();
                    std::process::exit(1);
                }
            }
        }
        return Ok(());
    }

    let mut shell = Shell::new(true)?;
    shell.run_repl()?;

    Ok(())
}

// Add this to shell module temporarily
impl Shell {
    pub fn run_repl(&mut self) -> Result<()> {
        println!(
            "{}",
            "WinSH MVP6 - Array Support and Internationalization".green()
        );
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

                    if let Err(e) = self.save_history(line) {
                        eprintln!(
                            "{} {}",
                            "Warning:".yellow(),
                            format!("Failed to save history: {}", e)
                        );
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
}

/// Initialize WinuxCmd daemon
fn initialize_winuxcmd_daemon() -> anyhow::Result<()> {
    println!("{}", "Initializing WinuxCmd daemon...".blue());

    // Initialize FFI first
    WinuxCmdFFI::init().map_err(|e| anyhow::anyhow!("{}", e))?;

    // Get version after FFI initialization
    println!("{} {}", "WinuxCmd version:".blue(), WinuxCmdFFI::get_version());
    println!("{} {}", "Protocol version:".blue(), WinuxCmdFFI::get_protocol_version());

    // Check if daemon is available
    if WinuxCmdFFI::is_available() {
        println!("{}", "✓ WinuxCmd daemon is available".green());
        return Ok(());
    }

    println!("{}", "✗ WinuxCmd daemon is not available, starting it...".yellow());

    // Start daemon process
    let daemon_exe = std::path::PathBuf::from("utils/winuxcmd/winuxcmd.exe");
    if !daemon_exe.exists() {
        return Err(anyhow::anyhow!("WinuxCmd executable not found at: {:?}", daemon_exe));
    }

    let mut daemon_cmd = std::process::Command::new(&daemon_exe);
    daemon_cmd.arg("--daemon");
    daemon_cmd.stdout(std::process::Stdio::piped());
    daemon_cmd.stderr(std::process::Stdio::piped());

    let mut child = daemon_cmd.spawn()
        .map_err(|e| anyhow::anyhow!("Failed to start daemon: {}", e))?;

    // Wait for daemon to start (give it 5 seconds)
    for i in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if WinuxCmdFFI::is_available() {
            println!("{}", "✓ WinuxCmd daemon started successfully".green());
            return Ok(());
        }
    }

    // If daemon still not available, check child process status
    let status = child.try_wait()?;
    if let Some(exit_status) = status {
        return Err(anyhow::anyhow!("Daemon exited with status: {:?}", exit_status));
    }

    Err(anyhow::anyhow!("Daemon failed to start within timeout"))
}
