//! The tests every implementation of wcontainers-processor-cores runs, written against its traits alone.

use std::sync::Mutex;
use std::thread;

use wcontainers_processor_cores::ProcessorCores;

/// Checks that the machine has at least one core and that listing again gives the same identities.
pub fn lists_the_same_cores_again<Cores: ProcessorCores>(cores: Cores) {
    let (Ok(first), Ok(second)) = (cores.list_cores(), cores.list_cores()) else {
        panic!("the cores could not be listed");
    };
    assert!(!first.is_empty() && first.len() == second.len());
    for ((_, identity), (_, identity_again)) in first.iter().zip(&second) {
        assert!(identity == identity_again);
    }
}

/// Checks that the work runs on the calling thread and that what it returned is returned.
pub fn runs_the_work_on_the_calling_thread<Cores: ProcessorCores>(cores: Cores) {
    let Ok(listed) = cores.list_cores() else {
        panic!("the cores could not be listed");
    };
    for (core, _) in &listed {
        let caller = thread::current().id();
        let Ok(worker) = cores.run_on_core(core, || thread::current().id()) else {
            panic!("the thread could not be held on the core");
        };
        assert!(worker == caller);
    }
}

/// Checks that the work runs once for each listed core, each run on a thread of its own with that core's identity, and that what each run returned is returned.
pub fn runs_the_work_once_for_each_listed_core<Cores: ProcessorCores>(cores: Cores) {
    let Ok(listed) = cores.list_cores() else {
        panic!("the cores could not be listed");
    };
    let started = Mutex::new(Vec::new());
    let Ok(returned) = cores.run_on_all_cores(|identity| {
        let thread = thread::current().id();
        started.lock().unwrap().push(thread);
        (identity, thread)
    }) else {
        panic!("the work could not be run on the cores");
    };
    let started = started.into_inner().unwrap();
    assert!(returned.len() == listed.len() && started.len() == listed.len());
    for (position, ((identity, thread), (_, listed_identity))) in
        returned.iter().zip(&listed).enumerate()
    {
        assert!(identity == listed_identity);
        assert!(*thread != thread::current().id());
        assert!(started.contains(thread));
        assert!(
            !returned[..position]
                .iter()
                .any(|(_, other)| other == thread)
        );
    }
}

/// Runs each suite function above as its own test on a value made anew for every test.
///
/// List every suite function here, in the order above, and nothing else.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn lists_the_same_cores_again() {
            $crate::lists_the_same_cores_again($value);
        }

        #[test]
        fn runs_the_work_on_the_calling_thread() {
            $crate::runs_the_work_on_the_calling_thread($value);
        }

        #[test]
        fn runs_the_work_once_for_each_listed_core() {
            $crate::runs_the_work_once_for_each_listed_core($value);
        }
    };
}
