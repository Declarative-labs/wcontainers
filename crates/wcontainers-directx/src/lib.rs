//! The Direct3D family as one library: on Windows, `wcontainers_directx.dll`, whose exports are
//! Direct3D's entry points and whose COM objects wrap our fronts. Built for other targets it holds
//! nothing yet (the container module comes with the call channel).

#[cfg(windows)]
pub mod com;
