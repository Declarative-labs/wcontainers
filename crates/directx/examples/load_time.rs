//! Measures how long a fresh process takes to load a library (plan section 10's load time): each
//! argument is a library's path, loaded in turn; prints the microseconds each `LoadLibraryW` took.
//! Usage: `load_time <library>...`, one process per sample.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::System::LibraryLoader::LoadLibraryW;
    use windows::core::HSTRING;

    for path in std::env::args().skip(1) {
        let start = std::time::Instant::now();
        // SAFETY: loads the named library; it stays loaded until the process ends.
        unsafe { LoadLibraryW(&HSTRING::from(path.as_str()))? };
        println!("{path}: {} us", start.elapsed().as_micros());
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
