# Graphics rig status

Branch: phase-2-graphics-rig. The plan, docs/phase-2-graphics.md, is written on another branch and is
not in this checkout yet; this work follows the owner's brief (section "How it is built and proven").

## Where things stand

R0 (the Windows GPU machine set up, and the build, run and compare loop proven) is done and checked.
R1 (the Direct3D reference library) is partly done; its first run (2026-11-17) did this:
- Fixed the four findings of R0's check (see "R1 first run" under Measurements).
- Part 1, scenes, is done but for swap chains: 26 Direct3D 11 scenes (125 outputs) and 8 Direct3D
  12 scenes (16 outputs), each run on the RTX 3080 and on WARP with the debug layer on, the GPU's
  outputs stored as references with an allowance derived from WARP, the HLSL kept beside its
  compiled DXBC and DXIL. `tools/graphics-rig/scenes` checks every scene, through replacement
  libraries if given, and prints what passes and what differs.
- Part 2, Wine's Direct3D tests: the four test files (d3d11, d3d10core, dxgi, d3d12) of Wine 11.22
  build as 64-bit Windows programs in C:\wc\wine (`wine-tests build`) and run through the queue
  (`wine-tests run`), but every one of them stops within its first second: each creates a swap
  chain early, DXGI refuses it in session 0, and the test then crashes. No baseline yet.
- Part 3 (real programs and their call census) is not started.
- The blocker for the rest: jobs run in session 0, where DXGI refuses every swap chain (see Known
  issues). Wine's tests, a swap chain scene and real programs all need a session with a desktop.

The plan document, docs/phase-2-graphics.md, is on branch phase-2-graphics and not in this checkout;
its sections 10 to 12 (one source two builds, testing, milestones G4 and G5) were read for this run.

## How to run

All from the root of a checkout on the Mac. The desktop is tom@192.168.2.183 (key login).

- `tools/graphics-rig/setup-desktop`: installs the tools and the queue on the desktop and copies the
  desktop scripts to C:\wc\rig. Each step is skipped when done; run it again after changing
  anything in tools/graphics-rig/desktop.
- `tools/graphics-rig/remote <command>`: sends this checkout's working tree (untracked files
  included, ignored ones not) as a commit to C:\wc\src\<branch> and runs the command there with
  cmd.exe, with `CARGO_TARGET_DIR=C:\wc\target\<branch>`. Example:
  `tools/graphics-rig/remote cargo build -p wcontainers-graphics-test`. Runs of one branch take turns,
  and wait for that branch's running GPU jobs (each job holds the branch's lock shared).
- `tools/graphics-rig/gpu-job [--priority N] [--alone] [--build <command>] -- <command>`: runs a
  command as a GPU job in C:\wc\src\<branch>. That copy must hold this working tree as it is now; the
  job checks the git tree id when it starts and fails with 125 and a message if it differs.
  `--build <command>` sends the working tree and runs the build command there first, as `remote`
  does (not in a GPU slot), and the job then also fails with 125 unless the build directory still
  holds that build: `remote` removes the stamp the build writes (C:\wc\target\<branch>\built-tree)
  before each command. Use it whenever the job runs programs built from the tree, for example
  `gpu-job --build "cargo build -p wcontainers-graphics-test" -- "%CARGO_TARGET_DIR%\debug\gt-d3d11
  all hardware C:\wc\out\x"`. Without it the job runs whatever the build directory holds, which may
  have been built from an earlier tree. N is any integer, higher starts first; `--alone` waits until
  no other job runs and holds new ones back until it ends. The command is any cmd.exe command line,
  quotes and backslashes included. Prints the job's standard output on standard output and its
  standard error, the wrapper's messages and a timing line on standard error; exits with the job's
  code (125: the queue client or the job's wrapper failed, with a message saying which).
- `tools/graphics-rig/queue-check`: sends the working tree, then on an idle queue submits sleeping jobs
  and checks that two jobs run together, that of twice as many jobs as slots submitted together
  exactly as many as there are slots run at once, that a later priority-10 job starts before an
  earlier priority-0 one when every slot is taken, and that an alone job runs with nothing beside it.
  About 40 s.
- `tools/graphics-rig/loop [picture-dir]`: one `gpu-job --build` that builds the rig on the desktop
  (and reinstalls the browser driver's packages when package-lock.json's hash changed), then
  (tools/graphics-rig/loop.cmd) checks every scene as `scenes` does, checks triangle, quads and
  topology again through the forwarding d3d11.dll (`--dll d3d11.dll=...forward.dll`), and draws the
  triangle with wgpu (Direct3D 12, Vulkan) and Chrome (WebGPU, WebGL 2), each of which must equal
  Direct3D 11's at tolerance 0. It fails unless the forwarding run loaded d3d11.dll from its own
  directory and logged "d3d11 forward: D3D11CreateDevice" (the program's device creation went
  through the replacement), and unless Chrome's WebGPU adapter is a GPU (type discrete or
  integrated, not a fallback) and its WebGL 2 renderer names the same GPU. gt-d3d11 and gt-d3d12 fail when a hardware device lands on a
  software adapter (DXGI's software flag: WARP or the Microsoft Basic Render Driver), and gt-wgpu
  when its adapter is a CPU one. With a directory, the pictures are also copied there.
- `tools/graphics-rig/scenes [--build <command>] [--dll <name>=<path>]... [--api d3d11|d3d12]
  [--driver hardware|warp] [<scene>...]`: one GPU job that builds the rig, copies gt-d3d11.exe and
  gt-d3d12.exe into C:\wc\out\<branch>\scenes\bin with each `--dll` beside them under its name
  (paths on the desktop, relative to its checkout; `%CARGO_TARGET_DIR%` works), runs every scene (or
  the named ones) in its own process with a 120 s limit, and checks every output against
  crates/wcontainers-graphics-test/references/<api>. Prints `pass` or `FAIL` and the numbers for each
  output, a FAIL for a scene that crashed, hung, has no references, or did not load a replacement
  library from that directory, whether the stored shader corpus matches the build, and a count.
  Exits 1 on any FAIL. Pointing it at our own Direct3D: `scenes --build "cargo build -p <crate>"
  --dll d3d11.dll=%CARGO_TARGET_DIR%\debug\<its dll> --dll dxgi.dll=...`. The scene programs do not
  ask for the debug layer when checking, so a replacement need not provide it.
- `tools/graphics-rig/scenes --record`: runs every scene with the debug layer on the GPU twice and on
  WARP; fails if the two GPU runs differ anywhere or the debug layer reports an error; then copies
  the GPU's outputs and references/<api>/tolerances.txt (each picture's allowance and how far WARP
  was) and the compiled shaders into this checkout, to be committed. Prints WARP's difference for
  every output. Run it after changing or adding a scene, look at the new pictures, commit.
- How a picture passes: it is compared at its own depth (8-bit, 16-bit or float) with the GPU's;
  with L the largest channel difference WARP had from the GPU, the tolerance is L capped at 8 steps
  of an 8-bit channel (2056 of a 16-bit one, 1/128 of a float one), and the pixels allowed beyond it
  are as many as WARP had beyond it: a replacement passes when it is at least as close to the GPU as
  WARP is. Texts (query counts, read-back bytes, answers to capability questions) must match exactly
  and hold only facts every correct implementation shares (timestamps are recorded as facts: in
  order, not disjoint; pixel shader invocations as "at least the covered pixels").
- Scenes are listed by `gt-d3d11 list` and `gt-d3d12 list`; a scene alone, on the desktop:
  `gt-d3d11 <scene>... <hardware|warp> <dir> [--debug]` writes `<scene>[-<part>].<png|exr|txt>`.
  The scene sources are crates/wcontainers-graphics-test/src/d3d11/scenes/*.rs and
  src/d3d12/scenes.rs; each file's first comment says what it covers.
- Shaders: crates/wcontainers-graphics-test/shaders/*.hlsl, each naming its entry points in header
  lines `// fxc: <target> <entry>, ...` (DXBC for Direct3D 11) and `// dxc: ...` (DXIL, shader model
  6.0, for Direct3D 12). build.rs compiles them with the newest Windows SDK's fxc.exe and dxc.exe
  (10.0.26100.0; dxc 1.9.2602.17) when building for Windows; the programs embed the bytecode. The
  corpus for the shader track is shaders/dxbc (79 files) and shaders/dxil (14 files), written by
  `scenes --record`; `scenes` says when it no longer matches the build.
- `tools/graphics-rig/wine-tests build`: fetches llvm-mingw 20261104 and the Direct3D test sources
  of Wine 11.22 (commit 9f8c4ae4d35c7ff4153267aba9618c026f5f5583, sparse: the four test
  directories, libs/winecrt0, include/wine, include/ddk) into C:\wc\wine and builds
  C:\wc\wine\build\<test>_test.exe for d3d11, d3d10core, dxgi and d3d12 (desktop/wine-tests.ps1).
  Not a GPU job. About 50 s from nothing.
- `tools/graphics-rig/wine-tests run [--warp] [<test>...]`: one GPU job running the named test files
  (all four by default; a test file is Wine's unit of selection, its framework has no way to pick
  one test function) on the GPU or on WARP; prints for each its summary line, failed-check count,
  exit code and time; logs in C:\wc\out\<branch>\wine and copied to the scratch directory
  (wine-logs). Exits 1 if any test failed or crashed.
- `tools/graphics-rig/inventory`: prints the desktop's installed programs, root scheduled tasks,
  services, Graphics Tools state, PATHs and C:\wc, to compare with "The desktop" below. The listing
  taken before anything was installed is in the scratch directory
  (pyro-parked/wcontainers-scratch/graphics-rig/baseline/programs-before.txt and state-before.txt),
  not in this public repository, as it lists the owner's software.
- Other renderers of the triangle, on the desktop (in a job): `gt-wgpu <dx12|vulkan|gl> <dir>`,
  `gt-scene triangle > triangle.json` then `node tools\graphics-rig\browser\draw.mjs triangle.json
  <dir>` (the page takes the triangle's size, clear colour and vertices from scene.rs through
  gt-scene, and the shader from triangle.wgsl). `gt-wgpu metal <dir>` also runs on the Mac.
- `gt-compare <a> <b> [tolerance]`: compares two pictures at their own depth (PNG 8 or 16-bit,
  OpenEXR float; pictures of different depths are refused); the tolerance is in the pictures' units
  (steps of a channel, or a float value). Prints pixel count, how many differ in any channel by more
  than the tolerance, the largest channel difference and the first differing pixel. Exit 0 when
  none differ beyond the tolerance, 1 when some do, 2 on error.
- Tests: `cargo test -p wcontainers-graphics-test -p wcontainers-graphics-test-forward` (6 unit tests:
  comparison at each depth, NaN, the WARP allowance) on the Mac or through `remote`; clippy with `-D warnings` on both machines,
  since most of the code is Windows-only; `shellcheck -x tools/graphics-rig/*` (minus the .cmd) for
  the Mac scripts.
- The Mac scripts share tools/graphics-rig/common.sh: every ssh and scp to the desktop goes through one
  shared connection per checkout (control socket ~/.ssh/wc-rig-<hash of the git dir>, kept 10 minutes
  after last use), opened first with a retry every 15 s for up to 20 minutes, so a run started while
  the desktop refuses new connections waits instead of failing.

## The desktop

Windows 10 Home 19045, i9-9900K, 32 GB, RTX 3080 (driver 32.0.16.2101, NVIDIA 621.01). Everything
of ours is under C:\wc, except what installers put in their usual places (listed here).

Installed (by tools/graphics-rig/desktop/install-tools.ps1, direct installers). Every version is
pinned except the Visual Studio Build Tools: Microsoft's bootstrapper (aka.ms/vs/17/release) installs
the current 17.x, and no fixed-version bootstrapper URL was found. install-tools.ps1 prints the
versions it finds and writes them to C:\wc\setup\versions.txt; on 2026-11-17 (R1): git
2.56.1.windows.1, node v26.13.1, Build Tools 17.14.45 with MSVC 14.44.35207 and Windows SDK
10.0.26100.0, rustup 1.29.3 with default toolchain 1.99.0, pueue 4.0.4, Chrome for Testing
157.0.8103.78.
- Git for Windows 2.56.1 (C:\Program Files\Git, 0.40 GB). Only Git LFS among the optional parts: no
  shell menu entries, icons or file associations. Adds C:\Program Files\Git\cmd to the machine PATH.
- Node.js 26.13.1 (C:\Program Files\nodejs, 0.10 GB). Adds itself to the machine PATH and
  %APPDATA%\npm (which does not exist) to the user PATH. Until R1, npm kept its cache in the
  owner's profile (C:\Users\tom\AppData\Local\npm-cache, 3.4 MB) and Node its compile cache in
  his Temp folder (node-compile-cache, 1.4 MB); both were removed in R1. Now every rig script on the
  desktop (remote.ps1, submit.ps1 for jobs, install-tools.ps1, browser\install.ps1) dot-sources
  desktop/env.ps1, which sets TEMP and TMP to C:\wc\tmp, npm's cache to C:\wc\npm-cache and Node's
  compile cache to C:\wc\tmp\node-compile-cache, so what the rig runs (cargo, the linker, npm,
  Chrome) writes its temporary files under C:\wc.
- Visual Studio Build Tools 2022 17.14 (MSVC 14.44.35207, workload VCTools with its recommended
  parts) with Windows SDK 10.0.26100: 3.51 GB in C:\Program Files (x86)\Microsoft Visual Studio,
  1.69 GB in C:\Program Files (x86)\Windows Kits. It also installed the Visual Studio Installer, about
  70 SDK and VS parts in the programs list, two manual-start services (VSInstallerElevationService,
  VSStandardCollectorService150), and upgraded the shared Visual C++ 2015-2022 x86 runtime from
  14.42.34438 to 14.44.35211 and added the x64 and x86 debug runtimes. Among the parts are
  vs_filehandler_amd64 and vs_filehandler_x86 (17.14.36024), vs_githubprotocolhandlermsi (17.14.36015)
  and vs_vswebprotocolselectormsi (17.14.36323): they register the URL protocols vsweb, vsweb+diag,
  vsweb+githubsi, vsweb+mcp, vsweb+teamstoolkit and git-client, about 150 VisualStudio.* file types
  (ProgIDs such as VisualStudio.cpp.72b06fc8 and VisualStudio.Launcher.sln), and make .sln files open
  with the Visual Studio launcher. Of .cpp, .h, .cs, .hlsl, .config, .dmp, .xml, .html, .htm, .bmp,
  .ico and .sln, only .sln has one of these as its default; the owner's chosen programs for .xml,
  .html, .htm and .bmp are unchanged. Installer caches outside its folders: C:\ProgramData\Package
  Cache 0.46 GB (70 folders created on 2026-11-17), C:\ProgramData\Microsoft\VisualStudio\Packages
  0.04 GB, C:\Windows\Installer 0.08 GB, and files created that day in C:\Windows\WinSxS (0.14 GB),
  System32 (0.05 GB) and SysWOW64 (0.03 GB), from these installs and Graphics Tools.
- rustup 1.29.3 (pinned, from rustup's archive) with RUSTUP_HOME=C:\wc\rustup and
  CARGO_HOME=C:\wc\cargo (user variables); C:\wc\cargo\bin is on the user PATH. One toolchain,
  1.99.0 (the workspace's rust-toolchain.toml channel), with its wasm32 and aarch64-musl targets, is
  also the default; the stable 1.100.0 installed first was removed in R1.
- Windows' optional feature Graphics Tools (Tools.Graphics.DirectX~~~~0.0.1.0): the Direct3D 11 and
  12 debug layers (d3d11_3SDKLayers.dll, d3d12SDKLayers.dll). A one-off scheduled task as SYSTEM
  added it (see Departures) and was removed after.
- pueue 4.0.4 (pueue.exe, pueued.exe in C:\wc\bin), MIT. The scheduled task wc-gpu-queue starts
  `pueued -c C:\wc\pueue\pueue.yml` at boot as tom (logon type S4U, so in session 0, without a window,
  whether or not anyone is logged in). Queue state and logs are in C:\wc\pueue.
- Chrome for Testing 157.0.8103.78 in C:\wc\chrome (0.43 GB), pinned, downloaded by
  @puppeteer/browsers; draw.mjs refuses to run if C:\wc\chrome holds any other build.
  Not installed into Windows: no updater, no services, no shortcuts. Never signed in.

C:\wc holds, besides the tools above (bin, cargo, chrome, rustup, pueue): the installers in
downloads (0.12 GB), the Graphics Tools task's log in setup, checkouts (C:\wc\src\<branch>, 0.02 GB),
build output (C:\wc\target\<branch>, 2.13 GB), pictures (C:\wc\out\<branch>), the desktop scripts
(C:\wc\rig), scripts in transit (C:\wc\incoming) and the branch locks (C:\wc\locks); 4.5 GB in all
when measured in R0. Since R1 also C:\wc\tmp (the rig's temporary files) and C:\wc\npm-cache.
Leftovers of the earlier measurements and checks (C:\wc\target\abi-x and measure-abi, C:\wc\out\queue
and s0, two export listings) were removed in the third run. Free space went from 521.7 GB before
anything was installed to 508.8 GB now, a drop of 12.9 GB. The items sized here add up to about
11.0 GB (C:\wc 4.5, Visual Studio 3.51, Windows Kits 1.69, Git 0.40, Node 0.10, the caches and system
files above 0.8, our part of the user's Temp folder 0.08); the other 1.9 GB are not accounted for
and may not be ours (the machine is in use, and C:\agent belongs to someone else).

The owner's Temp folder (C:\Users\tom\AppData\Local\Temp), measured in R1: 0.347 GB in all, of
which 0.083 GB were created from 2026-11-16 on, nearly all from our installs: the Visual Studio
installer's dd_*.log files and its windowssdk folder (about 0.07 GB), a WinGet folder from the failed
winget attempt (3.8 MB) and a 10 MB .tmp file created at 07:50 on the install day. The rest (two
127 MB wct*.tmp files of 2026-11-09 and 2026-11-11, and older files) was there before. The logs are
left in place.

Not installed: CMake and Ninja (nothing needs them yet). Nothing touched partitions, boot settings,
Windows Update settings, the GPU driver, games or the owner's files; no reboot. Seen between the
before and after listings but not done by anything of ours: the Windows Image Acquisition service
(stisvc) went from manual to automatic start. Found already there and left alone: C:\agent and its
scheduled tasks (AgentDesktop, AgentSys, HdriDownload, NvDriver), which belong to someone else.

## Measurements

All on the desktop above; desktop CPU load 1 to 5 % before each, Mac load average 10 to 22.

Remote builds (`remote`, wcontainers-abi with its dependencies, crates already downloaded, debug):
- cold, empty build directory: cargo 16.2 s, 17.9 s from the Mac.
- warm, nothing changed: cargo 0.07 s, 1.7 s from the Mac.
- warm, one line added to the crate: cargo 0.23 s, 1.8 s from the Mac.
- wcontainers-graphics-test first build (wgpu, windows, image): 1 min 23 s.
- `loop` with everything built: 9.6 to 11 s end to end; the GPU job itself 3.4 to 4.4 s.

GPU against WARP, the same scene and API (256 by 256, RGBA8 unorm):
- Direct3D 11 triangle: 1950 pixels differ, by at most 1 in a channel.
- Direct3D 11 quads: 3863 pixels differ, by at most 1.
- Direct3D 11 instanced: identical.
- Direct3D 12 triangle: 1950 pixels differ, by at most 1 (the same pixels as Direct3D 11).
So a tolerance of 1 is the smallest that accepts WARP; on the GPU itself, differences of 0 are what
was measured, so any tolerance above 0 against the GPU's own pictures hides real differences.

Every other renderer of the triangle against Direct3D 11 on the RTX 3080: identical, every pixel:
Direct3D 12, wgpu on Direct3D 12, wgpu on Vulkan, Chrome WebGPU, Chrome WebGL 2, and Direct3D 11
through the forwarding d3d11.dll. wgpu on Metal on the Mac (Apple M5 Max) differs in 1952 pixels by
at most 1.

Chrome 157, headless, in session 0 (the queue's session):
- WebGPU: "NVIDIA GeForce RTX 3080, D3D12, discrete GPU", not a fallback adapter (GPUAdapterInfo with
  --enable-webgpu-developer-features, which only adds the backend, type and name to it).
- WebGL 2: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 (0x00002206) Direct3D11 vs_5_0 ps_5_0, D3D11)".
- chrome://gpu lists Dawn adapters D3D12, Vulkan and D3D11 on the RTX 3080 first, and WARP and
  SwiftShader after; it reports "WebGPU: Hardware accelerated but at reduced performance" and
  compositing "Software only", which is headless Chrome not compositing on the GPU; drawing is on it.
- The first WebGPU adapter request just after Chrome starts failed in most runs ("A valid
  external Instance reference no longer exists"); a request made later succeeds. draw.mjs waits for
  the first success (at most 10 tries 200 ms apart) and prints how many failed.

Queue (`queue-check`, two runs, both all pass): two jobs submitted together started 12 ms apart and
ran together; with 4 slots full, a priority-10 job submitted 1 s after a priority-0 one started
first (07.700 against 08.958); an alone job's command began only after the two running jobs ended
and a job submitted meanwhile started after it. A queued job starts 0.1 to 0.3 s after submission
on an idle queue.

A change is caught: with the red of the triangle's first corner set to 0.9 in the browser page only,
`loop` failed: Chrome WebGPU and WebGL 2 each differed from Direct3D 11 in 18928 pixels, by up to 25.

Second run (desktop CPU about 4 %, Mac load average 11 to 16):
- `loop` with the forwarding DLL's copy removed from loop.cmd now fails (exit 1, "the forwarding run
  did not create its device through the replacement d3d11.dll"), though its picture still equals
  the direct one; with the copy it passes and logs the four calls. `findstr /x` cannot be used for
  this check: it never matches lines ending in LF alone, which is how Rust's eprintln ends them.
- With the triangle's first red set to 0.998 in scene.rs only, `loop` passes (every renderer, Chrome
  included, draws the changed triangle), and gt-compare against the earlier pictures finds 3260
  pixels differing by 1, for Direct3D 11 and Chrome WebGL 2 alike.
- Chrome's adapters, now checked by draw.mjs: WebGPU {"description":"NVIDIA GeForce RTX 3080",
  "backend":"D3D12","type":"discrete GPU","fallback":false}; WebGL 2 "ANGLE (NVIDIA, NVIDIA GeForce
  RTX 3080 (0x00002206) Direct3D11 vs_5_0 ps_5_0, D3D11)".
- `queue-check`, one run, all five rules pass: of 8 four-second jobs submitted together, 4 started at
  01.470 to 01.540 and the other 4 at 05.949 to 06.020, once the first ended (most at once: 4).
- `gpu-job --priority -2` runs (task at priority -2). A `gpu-job` after an untracked file was added
  on the Mac, without `remote`, failed with 125 and the tree message. A `remote` started 3 s into a
  7 s job waited ("waiting for another run") and ran 1 s after the job ended.
- `loop`, everything built: 18 s end to end with the remote build step; the GPU job itself 3.7 to
  5.9 s.

Third run (desktop CPU 1 %, Mac load average 28 to 33, other tracks building):
- `gpu-job` with the commands the check found broken: `echo "hello world"` prints "hello world";
  `dir "C:\Program Files\Git\cmd\git.exe" /b` prints git.exe; `echo trailing\` prints trailing\;
  a command whose 120-character label ends in a backslash runs; `echo hello out & echo hello err 1>&2
  & exit 7` gives "hello out" on standard output, "hello err" on standard error and exit 7.
- With the quads sampler set to point filtering, `loop` fails: d3d11-hardware-quads against
  reference-quads, 10728 pixels differ, by up to 94.
- Stale build: after building the point-filtering quads, reverting and `remote "exit 0"`, a job
  without `--build` still drew the point-filtered quads (equal to the earlier point picture); with
  `--build "cargo build -q -p wcontainers-graphics-test"` it drew the reverted ones (within 1 of the
  reference). With the queue full and a `--build` job waiting, a `remote "exit 0"` made the job fail
  with 125: "C:\wc\target\phase-2-graphics-rig holds a build of nothing, not of <tree>; another remote
  ran after this job's build".
- `queue-check`, one run, all five rules pass (8 jobs submitted together: 4 started at 20.436 to
  20.534, the other 4 at 24.941 to 25.028).
- `loop`, everything built: 9.0 s end to end including the build check; the GPU job itself 4.0 s.
- The software-adapter checks were not seen to fire: making the GPU's adapter fall back would mean
  disabling its driver, which this machine's rules forbid.

R1 first run (2026-11-17; desktop CPU load 1 to 4 % before each run, Mac load average 30 to 65):
- Carried findings fixed. Pictures are compared at their own depth: a 16-bit picture with +100 in
  one channel of one pixel now differs (unit test sixteen_bit_low_bits_count; 8-bit pictures as
  before); float targets are saved as OpenEXR and compared as floats (NaN equal only to NaN).
  The rig's npm cache, Node compile cache and temporary files now go under C:\wc (desktop/env.ps1);
  after `setup-desktop` and a `loop`, C:\Users\tom\AppData\Local\npm-cache and
  Temp\node-compile-cache are absent and C:\wc\npm-cache and C:\wc\tmp\node-compile-cache hold
  them. rustup-init (1.29.3), the toolchain (1.99.0) and Chrome (157.0.8103.78) are pinned; the
  Visual Studio bootstrapper is not, and its version is printed and kept (see The desktop). `loop`
  reinstalls the page driver's packages when package-lock.json's SHA-256 differs from the one
  stored in node_modules\.lock-hash.
- Scenes: every scene drawn twice on the GPU by `scenes --record` gave identical outputs (all 141).
  WARP against the GPU, per output, is in references/<api>/tolerances.txt. Most differ by at most 1
  to 2 steps of an 8-bit channel (or float rounding of about 1e-7 for depth and stream output);
  identical are, among others, instanced, the integer and float formats, copies, compute, the
  texture gather, msaa-load and the stencil values. Where WARP differs by more (allowance above 0
  pixels): GenerateMips at odd sizes (64 by 48: levels 5 and 6, by up to 33 steps; both answers
  are allowed by Direct3D), alpha to coverage (9396 of 16384 pixels, up to 108; the dither pattern
  is the implementation's), the sampler filter cells (29706 of 65536 pixels, up to 179;
  anisotropic filtering is the implementation's), and the BC blocks (8645 of 16384 texels, up to
  0.052; BC1 to BC3 interpolation precision is the implementation's). Texts identical on WARP but
  for pixel shader invocations (GPU 416, WARP 456 for a 416-pixel rectangle), now recorded as a
  lower bound.
- `scenes --record`: 15 s for the GPU job (all scenes three times with the debug layer). `scenes`
  (check only, all 34 scenes): about 10 s for the job. `loop`: 14.5 s end to end, 141 outputs plus
  the forwarding run's 3 scenes and the four other renderers of the triangle, all passing.
- A change is caught: with the blend constant's alpha set from 0.45 to 0.46, `scenes --api d3d11
  blend` failed blend-alpha (206 pixels beyond the allowed 2, by up to 3) and passed the other three
  blend pictures, which do not use that value.
- Through a replacement library: `loop` runs triangle, quads and topology with the forwarding
  d3d11.dll beside gt-d3d11 (all pass, and the program loaded d3d11.dll from its own directory).
- Wine's tests, first run on the RTX 3080 (2026-11-17): d3d11 stopped at d3d11.c:1776 after 2
  failed checks (hr 0x887a0022 creating a swap chain), then an access violation; d3d10core at
  d3d10core.c:1403 likewise; d3d12 at d3d12.c:635; dxgi at dxgi.c:6013 (output and swap chain
  checks). Each ran under 1.3 s. Not run on WARP yet; it would stop at the same calls.
- `cargo tree --workspace -e all` lists no Wine crate or source: the tests live only in C:\wc\wine
  on the desktop, fetched by the script.
- Swap chains in session 0 (the queue's session): CreateSwapChainForHwnd on a never-shown window with
  the flip model and with the blit model, and CreateSwapChainForComposition, all fail with
  DXGI_ERROR_NOT_CURRENTLY_AVAILABLE (0x887A0022). The scene is kept out of the list (its source is
  in the scratch directory, swapchain-scene.rs).

## Loading a replacement Direct3D library

How the forwarding works (crates/wcontainers-graphics-test-forward): build.rs reads the export table
of the build machine's C:\Windows\System32\d3d11.dll (with the object crate) and makes, for each of
its 51 exports, an x86-64 thunk exported under the same name and ordinal (link argument
`/EXPORT:name=thunk,@ordinal`; checked with dumpbin: same 51 names and ordinals). A thunk saves the
four integer and four vector argument registers, calls `resolve(index)`, which writes
"d3d11 forward: <name>" to standard error and finds the system function (cached), restores the
registers and jumps to it, so the caller's stack and stack arguments are untouched. A data export
or an export without a name would stop the build; d3d11 has neither.

Proof: loop.cmd copies gt-d3d11.exe and the built DLL (as d3d11.dll) into one directory and runs the
triangle there. The program prints "d3d11.dll from C:\wc\out\<branch>\forward\d3d11.dll, dxgi.dll
from C:\Windows\SYSTEM32\dxgi.dll" (GetModuleFileNameW of the loaded modules), the log shows
D3D11CreateDevice, D3D11CoreRegisterLayers, D3D11CoreGetLayeredDeviceSize and
D3D11CoreCreateLayeredDevice, and the picture equals the direct one. The picture alone proves nothing
(the system d3d11.dll draws the same), so loop.cmd fails unless both the path and the log show the
replacement.

Traps:
- Search order: d3d11.dll, d3d12.dll and dxgi.dll are not in the desktop's KnownDLLs list, so the program's own directory is searched
  first and a d3d11.dll there wins. A program that limits its search
  (SetDefaultDllDirectories with LOAD_LIBRARY_SEARCH_SYSTEM32) would not load it.
- The replacement must load the real library by full path (GetSystemDirectoryW + \d3d11.dll); by
  name, Windows returns the already loaded replacement itself.
- Other system libraries look d3d11.dll up by name and get the replacement too: the debug layer
  calls D3D11CoreRegisterLayers, D3D11CoreGetLayeredDeviceSize and D3D11CoreCreateLayeredDevice
  through it. A replacement must therefore serve calls from Windows' own libraries, not only from
  the program.
- The same applies to dxgi.dll, tried in the third run with a copy of the system dxgi.dll placed
  beside gt-d3d11: with the system d3d11.dll the program printed "d3d11.dll from
  C:\Windows\SYSTEM32\d3d11.dll, dxgi.dll from C:\wc\out\dxgi-trial\a\dxgi.dll", and with the
  forwarding d3d11.dll as well, both came from the program's directory. So the system d3d11.dll, loaded
  by full path, still takes dxgi.dll from the program's directory: a replacement dxgi.dll serves the
  real d3d11 too, and a replacement d3d11.dll that loads the system one gets the replacement dxgi.dll
  beneath it. The pictures were identical.
- A DLL built under the name d3d11 lands in the build directory beside every program built there, and
  all of them silently ran through it (seen in the first loop run). So the crate builds
  wcontainers_graphics_test_forward.dll, copied as d3d11.dll only where a test wants it.
- 64-bit only: the thunks are x86-64 code and build.rs reads the 64-bit System32 library; a 32-bit
  program would need an x86 build reading SysWOW64.

## Departures from the plan

There is no plan document in this checkout; these depart from the task or brief as written.
- winget could not be used: over SSH its source update fails (Appx deployment: "A specified logon
  session does not exist"). Each tool came from its official direct installer instead, with pinned
  versions in install-tools.ps1.
- Graphics Tools cannot be added from an SSH logon (Add-WindowsCapability and DISM: access denied),
  so a one-off scheduled task as SYSTEM adds it.
- Chrome is Chrome for Testing in C:\wc\chrome rather than Google Chrome installed into Windows: the
  same browser build, without an updater, services or entries in the owner's machine, and pinned.
  It is "current" only as of setup: `npx @puppeteer/browsers install chrome@stable --path C:\wc\chrome`
  fetches a newer one, and draw.mjs uses the newest present.
- The job queue client and the remote command are shell scripts in tools/graphics-rig (with PowerShell
  halves for the desktop), not Rust in the crate: they only drive ssh, scp and pueue.
- The queue tool is pueue (MIT): it runs on Windows, has groups with a parallel limit and per-job
  priorities. It has no exclusive mode, so an alone job runs in its own one-slot group and its
  wrapper (desktop/job.ps1) pauses the default group (letting running jobs finish), waits for them,
  runs, and resumes the group. Considered: HTCondor and Slurm (too heavy, Slurm is not on Windows),
  Windows Task Scheduler (no priorities or limits).
- pueue's own Windows service mode was not used: it reads its configuration from the user profile.
  A scheduled task passes -c C:\wc\pueue\pueue.yml, keeping it under C:\wc.
- Jobs run in session 0, not on the owner's desktop: Direct3D 11 and 12 hardware devices, wgpu and
  headless Chrome all reached the RTX 3080 there, and nothing can appear on the owner's screen.
- The forwarding DLL is a second crate, crates/wcontainers-graphics-test-forward, since a crate has one
  library and this one must be a cdylib with its own exports.
- The references are drawn with the Direct3D debug layer on, and any error it reports fails the run.
- The reference pictures are kept in the repository (three PNGs, 41 KB) so the loop catches a change
  that affects every renderer alike.
- Only the triangle is drawn by wgpu and Chrome, as R0 asked.
- The allowance for a replacement's picture is derived from WARP (as close to the GPU as WARP is,
  tolerance capped at 8 steps) rather than chosen per scene by hand, so every scene gets one rule;
  the record keeps WARP's raw numbers beside it.
- Shaders are compiled at build time by the Windows SDK's fxc.exe and dxc.exe rather than at run
  time by d3dcompiler_47.dll, so the programs carry fixed bytecode and the corpus in the repository
  is exactly what was tested. Direct3D 12 scenes use DXIL (shader model 6.0) only.
- The Direct3D 12 scenes cover its own model (root signatures of every parameter kind and both
  versions, tables, static samplers, placed and aliased resources, upload and readback heaps,
  bundles, barriers of all three kinds, ExecuteIndirect, queries and predication, three queues with
  fences, copies and resolves); the pipeline state each Direct3D 11 scene covers (blending, depth,
  stencil, formats and so on) is not repeated for Direct3D 12 yet.
- Wine's tests are built with llvm-mingw (clang and mingw-w64's headers and import libraries)
  rather than MSVC, each test file compiled alone in Wine's standalone test mode (STANDALONE,
  __WINESRC__), with Wine's include/wine headers, winecrt0's debug.c (the debug string helpers Wine's
  ntdll exports), and four Windows headers mingw-w64 lacks taken from Wine (ddk/d3dkmthk.h,
  d3dukmdt.h, dxcore.h, dxcore_interface.h); ARRAY_SIZE and DECLSPEC_EXPORT come from Wine's
  winnt.h, so they are defined on the command line, and an import library for d3d10_1.dll is made
  with llvm-dlltool. Wine's other headers are not used: half of them are generated from .idl files
  by Wine's build and clash with mingw-w64's. vkd3d's own Direct3D 12 tests are not built; Wine's
  dlls/d3d12/tests is.
- Texts hold only facts every correct implementation shares; values the implementation chooses
  (timestamps, helper pixel invocations, append order) are recorded as facts about them or sorted.

## Known issues

- No swap chain can be created in session 0, where the queue runs its jobs (see Measurements), so
  presenting is untested, Wine's tests crash early, and real programs, which present to a window,
  cannot run in the queue. Running them needs jobs in an interactive session with a desktop: the
  owner's own (his screen would show their windows while they run, and they would compete with his
  games), or a second Windows user's session (Windows 10 Home allows fast user switching, but such a
  session exists only while someone has logged that user in). This needs the owner's decision; until
  then parts 2 and 3 cannot be finished.

- SSH to the desktop has refused new connections (reset right after key exchange) for about 9 to 10
  minutes at a time and then recovered by itself, with no reboot and nothing in the OpenSSH or System
  logs. It happened twice in the first run (once after about 30 ssh sessions in 10 minutes) and
  again during the second independent check, when the checker had opened only about 4 new
  connections and an already open shared connection kept working throughout. So it is not this rig's
  connection count alone; other users of the machine may be the cause. Cause unknown; sshd_config sets
  no limits of its own. The scripts share one connection and wait up to 20 minutes for it (see How
  to run); in the third run the first connection was refused once and came up 15 s later.
- A job without `--build` runs whatever the build directory holds, which can be a build of an earlier
  tree; `--build` closes this. A `--build` job is still fooled if, between its build and its start,
  two other runs of the same branch build a different tree and send this one back (the second
  writes the stamp again); only runs of the same branch, normally one agent's, can do that.
- The stored references are pictures from this RTX 3080 and its driver (NVIDIA 621.01); a driver
  update may change them, and the loop would then fail at the reference comparison until they are
  checked and stored again.
- The branch lock is per branch: two agents sharing a branch still take turns correctly, but one
  agent's `remote` between another's `remote` and `gpu-job` makes the later job fail its tree check
  (it never runs the wrong code).
- If a `gpu-job` client is cut off, its job still runs; its output stays in
  C:\wc\pueue\data\task_logs and its standard error in C:\wc\pueue\errors, and the finished task
  stays listed until `pueue clean`.
- If the queue daemon restarts with jobs left, it pauses its groups; `setup-desktop` cleans those jobs
  and resumes the groups when it starts the daemon.
- An alone job pauses the default group and resumes it when it ends, also when its command fails;
  if its wrapper is killed (seen with `pueue kill --group alone`) the group stays paused until the
  next `gpu-job`, which resumes it when no alone job is queued or running and says so.
- A queued job starts PowerShell twice (pueue's shell runs desktop/job.ps1) before its command; a
  job of `exit 7` took 0.3 s from start to end.

## Licences

Added crates (all transitive dependencies are permissive: MIT, Apache-2.0, Zlib, ISC, BSD,
Unlicense-or-MIT, Unicode-3.0): in R1 image's exr feature (exr 1.74.2 BSD-3-Clause, with lebe
BSD-3-Clause, bit_field Apache-2.0/MIT, half 2.7.1 MIT OR Apache-2.0 (also used directly), pulp and
reborrow MIT, zune-inflate MIT OR Apache-2.0 OR Zlib), rand 0.9 and rand_chacha 0.9 (MIT OR
Apache-2.0, already in the workspace's lock file); in R0 anyhow, bytemuck (Zlib OR Apache-2.0 OR MIT), image, pollster, wgpu,
windows, windows-sys, object, serde_json 1.0.154 (already in the workspace's lock file), all MIT OR
Apache-2.0 unless said. Node packages (tools only):
playwright-core 1.65.1 and @puppeteer/browsers 3.2.7, Apache-2.0. Tools on the desktop, none shipped: in R1 llvm-mingw 20261104 (C:\wc\llvm-mingw, 0.70 GB;
its scripts ISC, LLVM Apache-2.0 with LLVM exceptions, mingw-w64 headers and runtime under their
permissive and public-domain terms) and Wine 11.22's test sources (C:\wc\wine, 0.16 GB with the
builds, LGPL-2.1, testing only, never in the repository or any crate); its download is in
C:\wc\downloads (now 0.30 GB);
pueue MIT, Git GPL-2.0, Node MIT, Rust MIT OR Apache-2.0, Visual Studio Build Tools and Windows SDK
under Microsoft's licence terms, Chrome for Testing under Google's terms.

## Next

R1 continues, in this order:
1. Settle where presenting jobs run (Known issues), then add that kind of job to the queue.
2. Part 2, Wine's Direct3D tests: run all four on the RTX 3080 and on WARP in that session and
   record the baseline (tests and checks run, failures on real Direct3D with their reasons, time).
3. Part 3, real programs, in that session too. Candidates to
   check at their source: DirectX-Graphics-Samples (MIT), DirectXTK samples (MIT), and two or three
   open-source games or demos with 64-bit Direct3D 11 renderers. For each: a fetch-or-build script
   into C:\wc, a fixed number of frames run repeatably, saved pictures, and a call census through a
   forwarding library that also sees the COM methods (R0's sees only the 51 exports).
4. Direct3D 12 versions of the pipeline-state scenes (blending, depth and stencil, formats,
   textures and samplers), and a swap chain scene once presenting can run.
