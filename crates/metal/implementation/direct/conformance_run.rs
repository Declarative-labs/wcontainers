#![cfg(target_os = "macos")]

wcontainers_metal_conformance::tests!(
    <wcontainers_metal_direct::DirectMetal as wcontainers_metal::StandaloneMetal>::new(());
    StandaloneMetal: yes
);
