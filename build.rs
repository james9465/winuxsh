// Build script for WinSH
// Handles DLL path configuration for WinuxCmd FFI

fn main() {
    // Output the path to winuxcmd.dll for runtime linking
    println!("cargo:rustc-link-search=native=utils/winuxcmd");
    println!("cargo:rustc-link-lib=dylib=winuxcmd");

    // For development, also check if DLL exists
    #[cfg(debug_assertions)]
    {
        let dll_path = std::path::Path::new("utils/winuxcmd/winuxcmd.dll");
        if !dll_path.exists() {
            println!(
                "cargo:warning=winuxcmd.dll not found at {}",
                dll_path.display()
            );
            println!("cargo:warning=FFI functionality will not be available");
        }
    }
}
