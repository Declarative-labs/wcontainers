#![cfg(target_os = "macos")]

wcontainers_test_window_conformance::tests!(<wcontainers_test_window_appkit::AppkitTestWindow as wcontainers_test_window::StandaloneTestWindow>::new(()); StandaloneTestWindow: yes);
