//! The process-wide host on a real host: when its zygote ends, every stub it held ends as killed, and the next fork
//! comes from a new zygote.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use std::time::{Duration, Instant};

use rustix::process::Signal;
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_executor_host_process::stub::TurnEnd;
use wcontainers_executor_host_process::zygote::Ended;

fn killed() -> Ended {
    Ended::Killed(Signal::KILL.as_raw().unsigned_abs())
}

#[test]
fn a_zygote_that_ends_takes_its_stubs_and_the_next_fork_comes_from_a_new_one() {
    let host = HostProcessHost::new().expect("the host is refused");
    let mut stubs: Vec<_> = (0..4).map(|_| host.fork_stub().expect("fork")).collect();
    let first = host.zygote_process().expect("a zygote");
    rustix::process::kill_process(first, Signal::KILL).expect("kill the zygote");
    for stub in &stubs {
        let deadline = Instant::now() + Duration::from_secs(5);
        while rustix::process::test_kill_process(stub.process()).is_ok() {
            assert!(Instant::now() < deadline, "a stub outlived its zygote");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(stub.turn(0, &[]), Ok(TurnEnd::StubEnded));
    }
    let stub = host.fork_stub().expect("a fork after the zygote's end");
    assert_eq!(host.zygote_ends(), 1);
    assert_ne!(host.zygote_process(), Some(first));
    assert_eq!(
        stub.turn(0, &[]),
        Ok(TurnEnd::Done {
            commands_applied: 0
        })
    );
    for old in stubs.drain(..) {
        assert_eq!(host.reap(&old), Ok(killed()));
    }
    host.kill(&stub).expect("kill");
    assert_eq!(host.reap(&stub), Ok(killed()));
    // A zygote that is alive is not forked anew.
    let again = host.fork_stub().expect("fork");
    assert_eq!(host.zygote_ends(), 1);
    assert_eq!(again.end(0), Ok(()));
    assert_eq!(host.reap(&again), Ok(Ended::Exited(0)));
}
