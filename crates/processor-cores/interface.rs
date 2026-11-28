use std::panic;
use std::thread;

use vstd::prelude::*;

verus! {

/// The cores of the machine's processors.
pub trait ProcessorCores: Sync {
    type Core: Sync;
    /// What a core says it is, read from the core itself.
    type CoreIdentity: Clone + Eq + Send + Sync;
    type ListCoresError;
    type RunOnCoreError: Send;

    fn list_cores(&self) -> Result<Vec<(Self::Core, Self::CoreIdentity)>, Self::ListCoresError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;

    /// Moves the calling thread onto the core for as long as the work runs, and returns what the work returned.
    fn run_on_core<Output, Work: FnOnce() -> Output>(
        &self,
        core: &Self::Core,
        work: Work,
    ) -> Result<Output, Self::RunOnCoreError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;

}

pub enum RunOnAllCoresError<ListCoresError, RunOnCoreError> {
    CoresNotListed(ListCoresError),
    CoreNotHeld(RunOnCoreError),
}

} // verus!

/// Performs run_on_core on all cores in parallel.
// VERUS: no proof of a body that runs threads, so this stays outside `verus!` as a function over any cores.
pub fn run_on_all_cores<Cores: ProcessorCores, Output: Send>(
    cores: &Cores,
    work: impl Fn(Cores::CoreIdentity) -> Output + Sync,
) -> Result<Vec<Output>, RunOnAllCoresError<Cores::ListCoresError, Cores::RunOnCoreError>> {
    let listed = cores
        .list_cores()
        .map_err(RunOnAllCoresError::CoresNotListed)?;
    thread::scope(|scope| {
        let runs: Vec<_> = listed
            .iter()
            .map(|(core, identity)| scope.spawn(|| cores.run_on_core(core, || work(identity.clone()))))
            .collect();
        runs.into_iter()
            .map(|run| {
                run.join()
                    .unwrap_or_else(|panic| panic::resume_unwind(panic))
                    .map_err(RunOnAllCoresError::CoreNotHeld)
            })
            .collect()
    })
}
