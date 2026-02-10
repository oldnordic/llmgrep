// Platform detection and feature flags
//
// Windows support is opt-in and explicit. This avoids silent behavior changes,
// bug reports from unsupported paths, and keeps trust with existing users.

#[cfg(feature = "windows")]
pub const IS_WINDOWS: bool = true;

#[cfg(not(feature = "windows"))]
pub const IS_WINDOWS: bool = false;

#[cfg(feature = "unix")]
pub const IS_UNIX: bool = true;

#[cfg(not(feature = "unix"))]
pub const IS_UNIX: bool = false;

/// Warn users about Windows limitations on first run
pub fn check_platform_support() {
    if IS_WINDOWS {
        eprintln!("=== Windows Support Notice ===");
        eprintln!("llmgrep on Windows is fully supported for analysis.");
        eprintln!();
        eprintln!("llmgrep is a read-only search tool with no background");
        eprintln!("processes, so Windows support is complete.");
        eprintln!("==================================");
    }
}
