//! Helpers for validating and checking names.

use cairo_lang_filesystem::db::CORELIB_CRATE_NAME;

pub const DEFAULT_TESTS_PATH: &str = "tests";

/// Checks if name is restricted on Windows platforms.
pub fn is_windows_restricted(name: &str) -> bool {
    [
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ]
    .contains(&name)
}

/// Checks if name equals `core` or `starknet`
pub fn is_internal(name: &str) -> bool {
    [
        CORELIB_CRATE_NAME,
        DEFAULT_TESTS_PATH,
        "test_plugin",
        "starknet",
    ]
    .contains(&name)
}
