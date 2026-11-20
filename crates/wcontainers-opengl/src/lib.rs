//! A Gallium driver for Mesa's OpenGL state tracker, drawing through `wcontainers-gpu`. Mesa's WGL
//! target links it as a static library (`mesa/patches/wcontainers-gallium-driver.patch` of
//! wcontainers-nir); it is written against bindings of the pinned Mesa's Gallium headers, made when
//! `WCONTAINERS_MESA` names its build.

#[cfg(mesa)]
pub mod device;
#[cfg(mesa)]
pub mod format;
#[cfg(mesa)]
pub mod gallium;
#[cfg(mesa)]
pub mod screen;
