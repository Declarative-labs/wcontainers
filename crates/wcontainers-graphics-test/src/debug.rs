//! What the Direct3D debug layers said about a run.

use anyhow::{Result, bail};

/// Prints the debug layer's messages, given as severity and text, and fails on any corruption or
/// error. Direct3D 11 and 12 number their severities alike: 0 corruption, 1 error, 2 warning,
/// 3 information, 4 message.
pub fn check(messages: Vec<(i32, String)>) -> Result<()> {
    let mut errors = 0;
    for (severity, text) in &messages {
        let name = match severity {
            0 => "corruption",
            1 => "error",
            2 => "warning",
            3 => "info",
            _ => "message",
        };
        eprintln!("debug layer {name}: {text}");
        errors += usize::from(*severity <= 1);
    }
    if errors > 0 {
        bail!("the debug layer reported {errors} errors");
    }
    Ok(())
}

/// The text of a message, without its closing nul.
pub fn text(description: *const u8, length: usize) -> String {
    let bytes = unsafe { std::slice::from_raw_parts(description, length) };
    String::from_utf8_lossy(bytes)
        .trim_end_matches('\0')
        .to_owned()
}
