//! A Gallium driver for Mesa's OpenGL state tracker, drawing through `wcontainers-gpu`. Mesa's WGL
//! target links it as a static library (`mesa/patches/wcontainers-gallium-driver.patch` of
//! wcontainers-nir); the Gallium side is the C bridge in `bridge/`, built against the pinned Mesa
//! named by `WCONTAINERS_MESA`.

#[cfg(mesa)]
pub mod device;
