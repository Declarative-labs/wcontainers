//! The EGL library stays loaded for the process once loaded from a folder (NEEDED "After GI7" item 7): alone in its
//! own test program, so that nothing else of the process has it loaded.

#[cfg(target_os = "linux")]
#[test]
fn the_library_stays_loaded_after_its_last_handle() {
    use wcontainers_egl::Egl;
    let folder = std::env::var_os("WCONTAINERS_EGL_FOLDER").map(std::path::PathBuf::from);
    let mapped = || std::fs::read_to_string("/proc/self/maps").unwrap_or_default().lines().any(|line| line.contains("libEGL"));
    assert!(!mapped(), "no EGL library is loaded before the test loads one");
    let library = wcontainers_egl_direct::Library::load(folder.as_deref()).expect("EGL loads from the folder named");
    assert!(mapped(), "the EGL library is loaded");
    drop(library);
    assert!(mapped(), "the EGL library stays loaded after its last handle");
}
