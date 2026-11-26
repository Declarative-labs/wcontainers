//! The recording back end built alone from its config (`StandaloneGpu`), as the suite builds them.

use crate::{Recorder, RecorderConfig};
use wcontainers_gpu::{caps::*, *};

#[test]
fn a_recorder_from_a_config_naming_no_caps_declares_what_one_made_directly_does() {
    let Ok(built) = <Recorder as StandaloneGpu>::new(RecorderConfig { caps: None });
    let direct = Recorder::new();
    let (a, b) = (built.caps().clone(), direct.caps().clone());
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    assert!(a.features.contains(Features::COMPUTE));
}

#[test]
fn a_recorder_declares_the_caps_its_config_names() {
    let mut caps = Recorder::new().caps().clone();
    caps.features.remove(Features::COMPUTE);
    caps.limits.vertex_streams = 3;
    let Ok(built) = <Recorder as StandaloneGpu>::new(RecorderConfig { caps: Some(caps) });
    let caps = built.caps().clone();
    assert!(!caps.features.contains(Features::COMPUTE));
    assert_eq!(caps.limits.vertex_streams, 3);
}
