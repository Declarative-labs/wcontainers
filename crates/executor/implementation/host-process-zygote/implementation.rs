//! Form P's zygote and stubs as the host runs them: where a stub's process holds its pages, the pages and messages the
//! embedder and a stub share, and the zygote's code, which runs in another process with none of the embedder's memory.

#[cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]
pub mod layout;
#[cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]
pub mod pages;
#[cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]
pub mod stub;
#[cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]
pub mod zygote;
