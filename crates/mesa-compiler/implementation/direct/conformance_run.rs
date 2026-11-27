//! The conformance suite on the direct crate, over the bridge and the pinned Mesa this crate's build links (only
//! where `WCONTAINERS_MESA` names Mesa's build and the `standalone` feature links it).
#![cfg(all(mesa, feature = "standalone"))]

wcontainers_mesa_compiler_conformance::tests!(<wcontainers_mesa_compiler_direct::Bridge as wcontainers_mesa_compiler::MesaCompiler>::new());
