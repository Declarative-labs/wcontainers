//! The executor interface's suite run on form P, each test with an executor of its own on one host per test process.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::sync::{Arc, OnceLock};

use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_executor_host_process::executor::HostProcess;
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_precise_instruction_set_translator::Translator;

static HOST: OnceLock<Arc<HostProcessHost>> = OnceLock::new();

fn executor() -> HostProcess {
    HostProcess::new(Arc::clone(HOST.get_or_init(|| {
        Arc::new(HostProcessHost::new().expect("the host is refused"))
    })))
}

wcontainers_executor_conformance::tests!(
    executor(),
    Translator,
    CriticalValueProduct::new(Arm64InputDomain)
);
