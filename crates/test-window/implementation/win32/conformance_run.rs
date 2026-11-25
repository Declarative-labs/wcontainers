#![cfg(windows)]

wcontainers_test_window_conformance::tests!(<wcontainers_test_window_win32::Win32TestWindow as wcontainers_test_window::StandaloneTestWindow>::new(()); StandaloneTestWindow: yes);
