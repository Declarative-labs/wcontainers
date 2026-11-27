//! One hostile case of the conformance suite alone over the bridge, a program of its own: WCONTAINERS_PROBE names
//! the case and the seed ("case,seed", or "x" for the number of cases); it fails without it, and where Mesa is not
//! linked.

#[cfg(all(mesa, feature = "standalone"))]
fn main() {
    wcontainers_mesa_compiler_conformance::probe_from_the_environment(<wcontainers_mesa_compiler_direct::Bridge as wcontainers_mesa_compiler::MesaCompiler>::new());
}

#[cfg(not(all(mesa, feature = "standalone")))]
fn main() {
    panic!("the probe needs Mesa linked: WCONTAINERS_MESA names its build and the standalone feature is on");
}
