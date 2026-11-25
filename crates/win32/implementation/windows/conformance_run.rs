#![cfg(windows)]

wcontainers_win32_conformance::tests!(<wcontainers_win32_windows::WindowsWin32 as wcontainers_win32::StandaloneWin32>::new(()); StandaloneWin32: yes);
