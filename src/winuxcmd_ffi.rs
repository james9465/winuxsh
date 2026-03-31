// WinuxCmd FFI bindings for WinSH
// Provides IPC client interface to WinuxCmd daemon

use std::ffi::{CStr, CString, c_char, c_int};
use std::path::Path;
use std::sync::Mutex;
use std::sync::Once;
use libloading::{Library, Symbol};

/// Response from WinuxCmd FFI
#[derive(Debug)]
pub struct WinuxCmdResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// FFI function types
type WinuxExecuteFunc = unsafe extern "C" fn(
    *const c_char,
    *const *const c_char,
    c_int,
    *const c_char,
    *mut *mut c_char,
    *mut *mut c_char,
    *mut usize,
    *mut usize,
) -> c_int;

type WinuxFreeBufferFunc = unsafe extern "C" fn(*mut c_char);

type WinuxIsDaemonAvailableFunc = unsafe extern "C" fn() -> c_int;

type WinuxGetVersionFunc = unsafe extern "C" fn() -> *const c_char;

type WinuxGetProtocolVersionFunc = unsafe extern "C" fn() -> c_int;

type WinuxGetAllCommandsFunc = unsafe extern "C" fn(
    *mut *mut *mut c_char,
    *mut c_int,
) -> c_int;

/// Global FFI library
static FFI_LIBRARY: Mutex<Option<Library>> = Mutex::new(None);

/// Safe wrapper for WinuxCmd FFI
pub struct WinuxCmdFFI;

static INIT: Once = Once::new();

impl WinuxCmdFFI {
    /// Initialize WinuxCmd FFI
    pub fn init() -> Result<(), String> {
        let mut result = Ok(());

        INIT.call_once(|| {
            let dll_paths = vec![
                "utils/winuxcmd/winuxcmd.dll",
                "./winuxcmd.dll",
                "../utils/winuxcmd/winuxcmd.dll",
            ];

            let mut last_error = String::new();

            for dll_path in dll_paths {
                match unsafe { Library::new(dll_path) } {
                    Ok(lib) => {
                        *FFI_LIBRARY.lock().unwrap() = Some(lib);
                        return;
                    }
                    Err(e) => {
                        last_error = format!("{}: {}", dll_path, e);
                        continue;
                    }
                }
            }

            result = Err(format!("Failed to load winuxcmd.dll from any location. Last error: {}", last_error));
        });

        result
    }

    /// Check if WinuxCmd is available
    pub fn is_available() -> bool {
        if let Some(ref lib) = *FFI_LIBRARY.lock().unwrap() {
            unsafe {
                let is_daemon_available: Symbol<WinuxIsDaemonAvailableFunc> = lib.get(b"winux_is_daemon_available").unwrap();
                is_daemon_available() != 0
            }
        } else {
            false
        }
    }

    /// Get WinuxCmd version
    pub fn get_version() -> String {
        if let Some(ref lib) = *FFI_LIBRARY.lock().unwrap() {
            unsafe {
                let get_version: Symbol<WinuxGetVersionFunc> = lib.get(b"winux_get_version").unwrap();
                let version_ptr = get_version();
                if version_ptr.is_null() {
                    "unknown".to_string()
                } else {
                    CStr::from_ptr(version_ptr)
                        .to_string_lossy()
                        .to_string()
                }
            }
        } else {
            "FFI not initialized".to_string()
        }
    }

    /// Get protocol version
    pub fn get_protocol_version() -> i32 {
        if let Some(ref lib) = *FFI_LIBRARY.lock().unwrap() {
            unsafe {
                let get_protocol_version: Symbol<WinuxGetProtocolVersionFunc> = lib.get(b"winux_get_protocol_version").unwrap();
                get_protocol_version()
            }
        } else {
            -1
        }
    }

    /// Get all available commands
    pub fn get_all_commands() -> Result<Vec<String>, String> {
        if let Some(ref lib) = *FFI_LIBRARY.lock().unwrap() {
            unsafe {
                let get_all_commands: Symbol<WinuxGetAllCommandsFunc> = lib.get(b"winux_get_all_commands")
                    .map_err(|e| format!("Failed to load winux_get_all_commands: {}", e))?;
                let free_buffer: Symbol<WinuxFreeBufferFunc> = lib.get(b"winux_free_buffer")
                    .map_err(|e| format!("Failed to load winux_free_buffer: {}", e))?;

                let mut commands_ptr: *mut *mut c_char = std::ptr::null_mut();
                let mut count: c_int = 0;

                let result = get_all_commands(&mut commands_ptr, &mut count);

                if result != 0 {
                    return Err(format!("Failed to get commands (error code: {})", result));
                }

                let mut commands = Vec::new();
                
                for i in 0..count {
                    let cmd_ptr = *commands_ptr.add(i as usize);
                    if !cmd_ptr.is_null() {
                        let cmd = CStr::from_ptr(cmd_ptr)
                            .to_string_lossy()
                            .to_string();
                        commands.push(cmd);
                        free_buffer(cmd_ptr);
                    }
                }
                free_buffer(commands_ptr as *mut c_char);

                Ok(commands)
            }
        } else {
            Err("FFI not initialized".to_string())
        }
    }

    /// Execute a command via WinuxCmd
    pub fn execute(command: &str, args: &[String]) -> Result<WinuxCmdResponse, String> {
        if let Some(ref lib) = *FFI_LIBRARY.lock().unwrap() {
            unsafe {
                let execute: Symbol<WinuxExecuteFunc> = lib.get(b"winux_execute")
                    .map_err(|e| format!("Failed to load winux_execute: {}", e))?;
                let free_buffer: Symbol<WinuxFreeBufferFunc> = lib.get(b"winux_free_buffer")
                    .map_err(|e| format!("Failed to load winux_free_buffer: {}", e))?;

                // Convert command to C string
                let command_cstr = CString::new(command)
                    .map_err(|e| format!("Invalid command name: {}", e))?;

                // Convert arguments to C strings
                let mut args_cstrings: Vec<CString> = Vec::new();
                let mut args_ptrs: Vec<*const c_char> = Vec::new();

                for arg in args {
                    let arg_cstr = CString::new(arg.as_str())
                        .map_err(|e| format!("Invalid argument: {}", e))?;
                    args_ptrs.push(arg_cstr.as_ptr());
                    args_cstrings.push(arg_cstr);
                }
                args_ptrs.push(std::ptr::null()); // Null-terminated array

                // Setup output buffers
                let mut output_ptr: *mut c_char = std::ptr::null_mut();
                let mut error_ptr: *mut c_char = std::ptr::null_mut();
                let mut output_size: usize = 0;
                let mut error_size: usize = 0;

                // Execute command
                let exit_code = execute(
                    command_cstr.as_ptr(),
                    args_ptrs.as_ptr(),
                    args.len() as c_int,
                    std::ptr::null(), // Use default cwd
                    &mut output_ptr,
                    &mut error_ptr,
                    &mut output_size,
                    &mut error_size,
                );

                // Extract output
                let stdout = {
                    if output_ptr.is_null() || output_size == 0 {
                        String::new()
                    } else {
                        let slice = std::slice::from_raw_parts(output_ptr as *const u8, output_size);
                        String::from_utf8_lossy(slice).to_string()
                    }
                };

                let stderr = {
                    if error_ptr.is_null() || error_size == 0 {
                        String::new()
                    } else {
                        let slice = std::slice::from_raw_parts(error_ptr as *const u8, error_size);
                        String::from_utf8_lossy(slice).to_string()
                    }
                };

                // Free buffers
                if !output_ptr.is_null() {
                    free_buffer(output_ptr);
                }
                if !error_ptr.is_null() {
                    free_buffer(error_ptr);
                }

                Ok(WinuxCmdResponse {
                    stdout,
                    stderr,
                    exit_code,
                })
            }
        } else {
            Err("FFI not initialized".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_winuxcmd_init() {
        let result = WinuxCmdFFI::init();
        assert!(result.is_ok(), "WinuxCmd FFI should initialize successfully");
    }

    #[test]
    fn test_winuxcmd_available() {
        let _ = WinuxCmdFFI::init();
        // This test depends on whether daemon is running
        let available = WinuxCmdFFI::is_available();
        println!("Daemon available: {}", available);
    }

    #[test]
    fn test_get_version() {
        let _ = WinuxCmdFFI::init();
        let version = WinuxCmdFFI::get_version();
        println!("WinuxCmd version: {}", version);
    }

    #[test]
    fn test_get_protocol_version() {
        let _ = WinuxCmdFFI::init();
        let protocol_version = WinuxCmdFFI::get_protocol_version();
        println!("Protocol version: {}", protocol_version);
        assert!(protocol_version >= 1, "Protocol version should be >= 1");
    }

    #[test]
    fn test_get_all_commands() {
        let _ = WinuxCmdFFI::init();
        // This test depends on whether daemon is running
        if WinuxCmdFFI::is_available() {
            let commands = WinuxCmdFFI::get_all_commands();
            assert!(commands.is_ok());
            let cmds = commands.unwrap();
            assert!(!cmds.is_empty());
            println!("Found {} commands", cmds.len());
        } else {
            println!("Daemon not available, skipping test");
        }
    }

    #[test]
    fn test_execute_simple_command() {
        let _ = WinuxCmdFFI::init();
        // This test depends on whether daemon is running
        if WinuxCmdFFI::is_available() {
            let response = WinuxCmdFFI::execute("pwd", &[]).unwrap();
            assert_eq!(response.exit_code, 0);
            assert!(!response.stdout.is_empty());
            println!("PWD: {}", response.stdout.trim());
        } else {
            println!("Daemon not available, skipping test");
        }
    }

    #[test]
    fn test_execute_with_args() {
        let _ = WinuxCmdFFI::init();
        // This test depends on whether daemon is running
        if WinuxCmdFFI::is_available() {
            let response = WinuxCmdFFI::execute("echo", &vec!["hello".to_string()]).unwrap();
            assert_eq!(response.exit_code, 0);
            assert!(response.stdout.contains("hello"));
            println!("Echo: {}", response.stdout.trim());
        } else {
            println!("Daemon not available, skipping test");
        }
    }
}