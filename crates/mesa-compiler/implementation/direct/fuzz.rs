//! The conformance suite's fuzzing over the bridge, a program of its own: WCONTAINERS_FUZZ_SEED and
//! WCONTAINERS_FUZZ_CALLS name the seed and the calls; it fails without them, and where Mesa is not linked.

#[cfg(all(mesa, feature = "standalone"))]
fn main() {
    wcontainers_mesa_compiler_conformance::fuzz_from_the_environment(<wcontainers_mesa_compiler_direct::Bridge as wcontainers_mesa_compiler::MesaCompiler>::new());
}

#[cfg(not(all(mesa, feature = "standalone")))]
fn main() {
    panic!("the fuzzing needs Mesa linked: WCONTAINERS_MESA names its build and the standalone feature is on");
}
