//! The recording back end built alone from its config (`StandaloneGpu`), as the suite builds them.

use crate::{Recorder, RecorderConfig};
use wcontainers_gpu::{caps::*, *};

#[test]
fn a_recorder_from_the_default_config_declares_what_one_made_directly_does() {
    let Ok(built) = <Recorder as StandaloneGpu>::new(RecorderConfig::default());
    let direct = Recorder::new();
    let (a, b) = (built.caps().clone(), direct.caps().clone());
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    assert!(a.features.contains(Features::COMPUTE));
}

#[test]
fn a_recorder_declares_the_caps_its_config_names() {
    let mut config = RecorderConfig::default();
    config.caps.features.remove(Features::COMPUTE);
    config.caps.limits.vertex_streams = 3;
    let Ok(built) = <Recorder as StandaloneGpu>::new(config);
    let caps = built.caps().clone();
    assert!(!caps.features.contains(Features::COMPUTE));
    assert_eq!(caps.limits.vertex_streams, 3);
}
