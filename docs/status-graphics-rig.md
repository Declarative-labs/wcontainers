# Graphics rig status

Branch: phase-2-graphics-rig. The plan, docs/phase-2-graphics.md, is written on another branch and is
not in this checkout yet; this work follows the owner's brief (section "How it is built and proven").

## Where things stand

R0 (the Windows GPU machine set up, and the build, run and compare loop proven) is done. The second
and third runs fixed what the independent checks found (listed under "Second run" and "Third run").
Everything below was run from this Mac against the desktop on 2026-11-17.

- The desktop has Git, Rust, the C++ build tools and Windows SDK, Node, Chrome for Testing, Windows'
  Graphics Tools (the Direct3D debug layers) and pueue, all listed under "The desktop".
- `remote` sends the working tree (committed or not) to the desktop and runs a command there.
- A pueue job queue on the desktop shares the GPU: short jobs by priority, at most 4 at once,
  timings alone. `gpu-job` submits (building first with `--build`), waits and returns standard
  output, standard error and exit code. `queue-check` proves the rules.
- crates/wcontainers-graphics-test draws three scenes (a coloured triangle; two textured,
  depth-tested quads with blending; a 16-instance draw placed and coloured from a constant buffer)
  with real Direct3D 11 on the RTX 3080 and on WARP, the triangle with Direct3D 12 on both, the
  triangle through wgpu (Direct3D 12 and Vulkan back ends) and in Chrome through WebGPU and WebGL 2,
  and compares pictures (`gt-compare`).
- crates/wcontainers-graphics-test-forward builds a d3d11.dll that logs each call of its 51 exported
  functions and forwards it to the system one (calls on the COM objects those return go through their
  vtables straight to the system library and are not seen); a scene program placed beside it runs
  through it and draws the same picture.
- The three Direct3D 11 scenes as drawn on the RTX 3080 are stored in
  crates/wcontainers-graphics-test/references, so a change to a scene is caught even when every
  renderer changes alike.
- `loop` does all of it in one command and fails if a picture leaves its tolerance, if the forwarding
  run did not load the replacement d3d11.dll or create its device through it, or if a GPU renderer
  (Direct3D 11 or 12 hardware, wgpu, Chrome) drew on a software adapter.

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
- `tools/graphics-rig/loop [picture-dir]`: one `gpu-job --build` that builds the rig on the desktop,
  then (tools/graphics-rig/loop.cmd) draws every scene with every renderer into C:\wc\out\<branch>,
  runs the forwarding test and compares: the GPU's Direct3D 11 pictures against the stored references
  (reference-<scene>.png) at tolerance 0, other GPU pictures of the triangle against the GPU's
  Direct3D 11 one at tolerance 0, GPU against WARP at tolerance 1. It also fails unless the
  forwarding run printed "d3d11.dll from <its own directory>\d3d11.dll" and logged
  "d3d11 forward: D3D11CreateDevice..." (the program's device creation went through the replacement),
  and unless Chrome's WebGPU adapter is a GPU (type discrete or integrated, not a fallback) and its
  WebGL 2 renderer names the same GPU. gt-d3d11 and gt-d3d12 fail when a hardware device lands on a
  software adapter (DXGI's software flag: WARP or the Microsoft Basic Render Driver), and gt-wgpu
  when its adapter is a CPU one. With a directory, the pictures are also copied there.
- Changing a scene on purpose: `loop <dir>` fails at the reference comparison; check the new
  pictures in <dir>, copy d3d11-hardware-<scene>.png over
  crates/wcontainers-graphics-test/references/reference-<scene>.png and commit them with the change.
- `tools/graphics-rig/inventory`: prints the desktop's installed programs, root scheduled tasks,
  services, Graphics Tools state, PATHs and C:\wc, to compare with "The desktop" below. The listing
  taken before anything was installed is in the scratch directory
  (pyro-parked/wcontainers-scratch/graphics-rig/baseline/programs-before.txt and state-before.txt),
  not in this public repository, as it lists the owner's software.
- Single renderers, on the desktop (in a job): `gt-d3d11 <triangle|quads|instanced|all>
  <hardware|warp> <dir>` (also prints which d3d11.dll and dxgi.dll it ran through),
  `gt-d3d12 <hardware|warp> <dir>`, `gt-wgpu <dx12|vulkan|gl> <dir>`,
  `gt-scene triangle > triangle.json` then `node tools\graphics-rig\browser\draw.mjs triangle.json
  <dir>` (the page takes the triangle's size, clear colour and vertices from scene.rs through
  gt-scene, and the shader from triangle.wgsl). `gt-wgpu metal <dir>` also runs on the Mac.
- `gt-compare <a.png> <b.png> [tolerance]`: prints pixel count, how many differ in any channel by
  more than the tolerance, the largest channel difference and the first differing pixel. Exit 0 when
  none differ beyond the tolerance, 1 when some do, 2 on error.
- Tests: `cargo test -p wcontainers-graphics-test -p wcontainers-graphics-test-forward` (2 unit tests
  of the comparison) on the Mac or through `remote`; clippy with `-D warnings` on both machines,
  since most of the code is Windows-only; `shellcheck -x tools/graphics-rig/*` (minus the .cmd) for
  the Mac scripts.
- The Mac scripts share tools/graphics-rig/common.sh: every ssh and scp to the desktop goes through one
  shared connection per checkout (control socket ~/.ssh/wc-rig-<hash of the git dir>, kept 10 minutes
  after last use), opened first with a retry every 15 s for up to 20 minutes, so a run started while
  the desktop refuses new connections waits instead of failing.

## The desktop

Windows 10 Home 19045, i9-9900K, 32 GB, RTX 3080 (driver 32.0.16.2101, NVIDIA 621.01). Everything
of ours is under C:\wc, except what installers put in their usual places (listed here).

Installed (by tools/graphics-rig/desktop/install-tools.ps1, direct installers):
- Git for Windows 2.56.1 (C:\Program Files\Git, 0.40 GB). Only Git LFS among the optional parts: no
  shell menu entries, icons or file associations. Adds C:\Program Files\Git\cmd to the machine PATH.
- Node.js 26.13.1 (C:\Program Files\nodejs, 0.10 GB). Adds itself to the machine PATH and
  %APPDATA%\npm (which does not exist) to the user PATH.
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
- rustup 1.29.3 with RUSTUP_HOME=C:\wc\rustup and CARGO_HOME=C:\wc\cargo (user variables);
  C:\wc\cargo\bin is on the user PATH. Stable 1.100.0 is the default; the checkout's
  rust-toolchain.toml brings 1.99.0 with its wasm32 and aarch64-musl targets. 1.75 GB.
- Windows' optional feature Graphics Tools (Tools.Graphics.DirectX~~~~0.0.1.0): the Direct3D 11 and
  12 debug layers (d3d11_3SDKLayers.dll, d3d12SDKLayers.dll). A one-off scheduled task as SYSTEM
  added it (see Departures) and was removed after.
- pueue 4.0.4 (pueue.exe, pueued.exe in C:\wc\bin), MIT. The scheduled task wc-gpu-queue starts
  `pueued -c C:\wc\pueue\pueue.yml` at boot as tom (logon type S4U, so in session 0, without a window,
  whether or not anyone is logged in). Queue state and logs are in C:\wc\pueue.
- Chrome for Testing 157.0.8103.78 in C:\wc\chrome (0.43 GB), downloaded by @puppeteer/browsers.
  Not installed into Windows: no updater, no services, no shortcuts. Never signed in.

C:\wc holds, besides the tools above (bin, cargo, chrome, rustup, pueue): the installers in
downloads (0.12 GB), the Graphics Tools task's log in setup, checkouts (C:\wc\src\<branch>, 0.02 GB),
build output (C:\wc\target\<branch>, 2.13 GB), pictures (C:\wc\out\<branch>), the desktop scripts
(C:\wc\rig), scripts in transit (C:\wc\incoming) and the branch locks (C:\wc\locks); 4.5 GB in all.
Leftovers of the earlier measurements and checks (C:\wc\target\abi-x and measure-abi, C:\wc\out\queue
and s0, two export listings) were removed in the third run. Free space went from 521.7 GB before
anything was installed to 508.8 GB now, a drop of 12.9 GB. The items sized here add up to about
11.0 GB (C:\wc 4.5, Visual Studio 3.51, Windows Kits 1.69, Git 0.40, Node 0.10, the caches and system
files above 0.8, the user's Temp folder 0.07); the other 1.9 GB are not accounted for and may not be
ours (the machine is in use, and C:\agent belongs to someone else).

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
- Only the triangle is drawn by Direct3D 12, wgpu and Chrome, as the task asks; the other two scenes
  are Direct3D 11 only so far.

## Known issues

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
Unlicense-or-MIT, Unicode-3.0): anyhow, bytemuck (Zlib OR Apache-2.0 OR MIT), image, pollster, wgpu,
windows, windows-sys, object, serde_json 1.0.154 (already in the workspace's lock file), all MIT OR
Apache-2.0 unless said. Node packages (tools only):
playwright-core 1.65.1 and @puppeteer/browsers 3.2.7, Apache-2.0. Tools on the desktop, none shipped:
pueue MIT, Git GPL-2.0, Node MIT, Rust MIT OR Apache-2.0, Visual Studio Build Tools and Windows SDK
under Microsoft's licence terms, Chrome for Testing under Google's terms.

## Next

R0 is complete. The next milestone comes from the plan once it is merged here.
