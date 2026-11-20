//! A Gallium driver for Mesa's OpenGL state tracker, drawing through `wcontainers-gpu`. Mesa's WGL
//! target links it as a static library (`mesa/patches/wcontainers-gallium-driver.patch` of
//! wcontainers-nir); it is written against bindings of the pinned Mesa's Gallium headers, made when
//! `WCONTAINERS_MESA` names its build.

#[cfg(mesa)]
mod blit;
#[cfg(mesa)]
mod context;
#[cfg(mesa)]
mod device;
#[cfg(mesa)]
mod draw;
#[cfg(mesa)]
mod format;
#[cfg(mesa)]
mod gallium;
#[cfg(mesa)]
mod indices;
#[cfg(mesa)]
mod outputs;
#[cfg(mesa)]
mod present;
#[cfg(mesa)]
mod query;
#[cfg(mesa)]
mod resource;
#[cfg(mesa)]
mod screen;
#[cfg(mesa)]
mod state;
#[cfg(mesa)]
mod stream;
#[cfg(mesa)]
mod view;
