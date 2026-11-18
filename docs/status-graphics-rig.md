# Graphics rig status

Branch: phase-2-graphics-rig. The plan, docs/phase-2-graphics.md, is written on another branch and is
not in this checkout yet; this work follows the owner's brief (section "How it is built and proven").

## Where things stand

R0 (the Windows GPU machine set up, and the build, run and compare loop proven) and R1 (the
Direct3D reference library) are done and checked. R2 (keeping the owner's PC safe, and what the
Direct3D 12 track waits for), three runs on 2026-11-17 and 18: the findings of R1's last check
and of R2's first check are fixed; the display driver resets are traced and the queue now
protects the PC from them, failed logons are recorded as they happen, Chrome's wrapper comes back
with every Chrome install, and every script keeps one SSH connection per branch (see "The owner's
PC" below). The forwarding library stands in for d3d12.dll as well as d3d11.dll: every Direct3D
12 scene passes through it, and it counts a Direct3D 12 program's device, queue, command list,
factory and swap chain calls and saves its frames, so bgfx's Direct3D 12 renderer, Godot's
Forward+ renderer on Direct3D 12 and two of Microsoft's Direct3D 12 samples are compared frame by
frame and have a census; the samples' census is checked against the calls their source makes.
R1's parts:
- Jobs that present run on a hidden desktop ("wc-rig") in the session of the user logged in at the
  console (`gpu-job --desktop`, see How to run): windows and swap chains work there and nothing
  shows on the owner's screen. Session 0, where the queue runs, refuses every swap chain.
- Part 1, scenes: 28 Direct3D 11 scenes (139 outputs) and 25 Direct3D 12 scenes (116 outputs).
  Every Direct3D 11 scene of drawing ground has a Direct3D 12 twin drawn from the same tables
  (the ones without are under Departures). Each output is recorded on the RTX 3080 twice and on
  WARP with the debug layer; `scenes` checks them, through replacement libraries if given. A
  picture passes when, in every 16 by 16 tile, it has no more pixels beyond the tolerance than WARP
  has there (How a picture passes). Each verdict is pass, FAIL or absent: absent when the device
  reports, through Direct3D's own queries, that it lacks what the output needs (a feature level,
  sample count, format ability or option); capability answers may be lower than the GPU's, never
  higher. The Direct3D 11 scenes ask for every level from 11_1 down; 24 of 28 run at 10_0 (the
  WebGL 2 route's level in the plan) and 1 more at 10_1, with parts needing more marked absent.
- Part 2, conformance tests: Wine's d3d11, d3d10core, dxgi and d3d12 test files and vkd3d's
  d3d12, d3d12_invalid_usage, hlsl_d3d12 tests and shader runner (8 files), each test in its own
  process, chosen by name; 19 Wine tests that would change the owner's screen are never run, and 1 that hangs the GPU runs on WARP only
  (wine-excluded.txt). Baselines on the RTX 3080 and WARP in tools/graphics-rig/wine-baseline. A
  test passes only if it has no failure and runs at least as many checks, and skips no more, than
  on real Direct3D.
- Part 3, real programs: 25 (5 DirectX SDK tutorials, 5 DirectX Tool Kit samples from Microsoft's
  Xbox-ATG-Samples, 6 bgfx examples that load no assets of their own on Direct3D 11 and 2 of them
  on Direct3D 12, 2 of Microsoft's Direct3D 12 samples (DirectX-Graphics-Samples), Chocolate Doom
  playing a Freedoom demo, Godot's 2D and 3D platformer demos on Direct3D 11 through ANGLE and on
  Direct3D 12), each run for 120 frames; frames 10, 60 and 120 are compared with references
  recorded on real Direct3D (kept on the desktop, their SHA-256 and allowances in the
  repository). Every program runs through the forwarding library as d3d11.dll or d3d12.dll,
  which takes the call census (tools/graphics-rig/census) and gives it a clock that moves by
  frames, so programs animated by the clock draw the same frames every run.

The plan document, docs/phase-2-graphics.md, is on branch phase-2-graphics and not in this checkout;
its sections 9 (feature levels: Direct3D 11 reports 11_0 on WebGPU and 10_0 on WebGL 2, Direct3D 12
11_0 on WebGPU only) and 10 to 12 were read for the eighth run.

## The owner's PC: display resets and failed logons

Display resets. On 2026-11-17 the display driver stopped responding and was reset 8 times (the
System log's Display 4101 events: 14:40:32, 15:25:53, 16:37:36, 16:38:47, 16:44:35, 17:22:13,
19:35:34, 20:37:38, Pacific time). Each reset blanks the screen for a moment and can crash a game.
What caused them, as far as the logs show:
- 6 of the 8 (14:40, 15:25, 16:37, 16:38, 17:22, 20:37): Wine's dxgi test program dxgi_test.exe on
  the real GPU. At each of these Windows wrote a Direct3D 12 "device removed" report (WER event
  D3DDRED2) naming dxgi_test.exe and the reason DXGI_ERROR_DEVICE_HUNG: its Direct3D 12 device hung
  the GPU. For 20:37 the report's path is this branch's copy (C:\wc\out\phase-2-graphics-rig\wine\bin)
  and the test running then was test_swapchain_backbuffer_index (its log was written from 20:37:36.7
  to 20:37:39.8); the webgl track's job logged beside it was a bystander. For the other five the
  reports are no longer kept, so which branch's run and which dxgi test is not known; the same
  program and the same hang make that test the likely one. Several branches ran Wine's tests that
  afternoon.
- 19:35: the deliberate break of R1's ninth check (a Direct3D 11 program made to hang the GPU, to see
  that jobs notice resets). Direct3D 11 writes no such report.
- 16:44: no report names a program, and the queue kept no history then, so the cause is not known.
  In the minute before, the webgpu track's test against_webgpu crashed inside ANGLE (16:43:44), and
  right after the reset the Direct3D 12 track's vkd3d tests through its own d3d12.dll crashed
  (16:44:36); either may have caused it.
What now prevents a repeat (desktop/hazard.ps1, in C:\wc\rig, which every branch's jobs go through):
- The kinds seen to reset the driver are Direct3D 12 conformance tests on the GPU (Wine's dxgi
  tests, six times), the rig's own scene program after an edit (19:35), and, for the unknown
  16:44, other tracks' own code. Jobs of these kinds run one at a time, in the queue group
  "hazard". The queue itself decides, not the job's sender: a job runs beside others only if its
  command is one of the rig's own runs with no replacement library (programs, scenes, the loop,
  Wine's tests on WARP) and the rig's code in its tree (the two test crates and
  tools/graphics-rig) has already run one job without a reset (C:\wc\logs\cleared.txt). So
  Wine's and vkd3d's tests on the GPU, replacement libraries, every other track's own command and
  the rig's own runs after any change to the rig run one at a time. `--reference` is ignored.
- A reset during a job fails it and holds the queue: no new GPU job starts, and gpu-job refuses new
  ones with the reason, until someone records the cause with `tools/graphics-rig/resets record
  "<cause>"`. Running jobs finish.
- What ran at the reset is quarantined until lifted (`resets lift`): the job's command, or, for
  Wine's tests on the GPU, the very test that was running (the run then stops). A command is
  refused when the words of a quarantined command all come in it in the same order, paths taken
  by their last part only, so another output directory or checkout, or anything added, does not
  get around it. A quarantined Wine test is skipped on the GPU and still runs on WARP.
- Commands sent with `remote` are watched the same way: refused while quarantined or while the
  queue is held, and a reset during one fails it, holds the queue and quarantines it.
- dxgi test_swapchain_backbuffer_index is kept off the GPU by the repository itself
  (wine-excluded.txt, marked "(gpu)"; it runs on WARP: 398 checks, the 154 failed ones being
  Present's "occluded" answers, as recorded), besides its quarantine on the desktop.
- C:\wc\logs\jobs.log records every GPU job's start and end (branch, tree, group, exit code,
  command), so the next reset can be matched to the jobs that ran.

Failed logons. The account was locked twice on 2026-11-17 by failed logons from our jobs; one cause
was Chrome checking for a blank password on each new profile (fixed by the wrapper in C:\wc\chrome),
the other was never found because the Security log had been overwritten. Now:
- The scheduled task wc-failed-logons writes every failed logon (event 4625) and lockout (4740) to
  C:\wc\logs\failed-logons.log as it happens: time, account, logon type, status, the process and
  its pid, workstation and source address, and the process's command line and parent while it still
  runs. It only reads the Security log; no security or audit setting was changed (the lockout
  threshold stays at 10).
- desktop/install-chrome.ps1 puts the Chrome wrapper (gt-chrome, crates/wcontainers-graphics-test/
  src/bin/gt-chrome.rs, the source from C:\wc\sshdiag\chrome-wrapper) in front of Chrome whenever it
  installs or updates Chrome, and whenever the wrapper there is not the current build.
- Every Mac script reaches the desktop through one long-lived SSH connection per branch (control
  socket ~/.ssh/wc-rig-<branch>, kept an hour after its last use). Only one script of a branch opens
  it; after a refused attempt it waits 30 s, then twice as long each time, at most 5 minutes: at most
  five attempts in ten minutes, where ten failed logons in ten minutes lock the account.

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
- `tools/graphics-rig/gpu-job [--priority N] [--alone] [--desktop] [--build <command>] -- <command>`: runs a
  command as a GPU job in C:\wc\src\<branch>. That copy must hold this working tree as it is now; the
  job checks the git tree id when it starts and fails with 125 and a message if it differs.
  `--build <command>` sends the working tree and runs the build command there first, as `remote`
  does (not in a GPU slot), and the job then also fails with 125 unless the build directory still
  holds that build: `remote` removes the stamp the build writes (C:\wc\target\<branch>\built-tree)
  before each command. Use it whenever the job runs programs built from the tree, for example
  `gpu-job --build "cargo build -p wcontainers-graphics-test" -- "%CARGO_TARGET_DIR%\debug\gt-d3d11
  all hardware C:\wc\out\x"`. Without it the job runs whatever the build directory holds, which may
  have been built from an earlier tree. N is any integer, higher starts first; `--alone` waits until
  no other job runs and holds new ones back from its submission until it ends. The queue puts
  every job in the group hazard, one at a time, except the rig's own runs on cleared rig code (The
  owner's PC, above); `--reference` is accepted and ignored. A quarantined command, or any job
  while the queue is held, is refused with 125 and the reason. The command is any cmd.exe command line,
  quotes and backslashes included. `--desktop` runs the command on the hidden desktop wc-rig of the
  session of whoever is logged in at the desktop's console, for anything that creates a window or
  a swap chain: the job (in session 0) writes the command, its directory and its whole environment
  to C:\wc\desktop\<guid>, starts the scheduled task wc-desktop (interactive logon, so it runs in
  that session) with that directory, and gt-desktop.exe runs the command there on wc-rig, with its
  output in files the job then prints; leftover processes are killed when the command ends. It
  fails after 30 s with a message when nobody is logged in. Prints the job's standard output on standard output and its
  standard error, the wrapper's messages and a timing line on standard error; exits with the job's
  code (125: the queue client or the job's wrapper failed, with a message saying which). A job
  during which the System log got a Display or nvlddmkm warning or error (event 4101: the display
  driver stopped responding and was reset, which blanks the owner's screen for a moment) fails
  (exit 1 if the command gave 0) with "job: FAIL, the display driver was reset ..." and the events,
  whichever job beside it caused it, and the events are added to C:\wc\logs\display-resets.log.
- `tools/graphics-rig/queue-check`: sends the working tree, then on an idle queue submits sleeping jobs
  (desktop/sleep.ps1, a rig run; the first, one at a time, clears the tree's rig code) and checks that two jobs run together, that of twice as many jobs as slots submitted together
  exactly as many as there are slots run at once, that a later priority-10 job starts before an
  earlier priority-0 one when every slot is taken, that an alone job runs with nothing beside it,
  and that two hazard jobs submitted together run one after the other. About 1 min on an idle
  queue; other tracks' hazard jobs make the alone rule wait for them.
- `tools/graphics-rig/resets`: prints whether the queue is held, the quarantined names and the end
  of C:\wc\logs\display-resets.log. `resets record "<cause>"` records what caused the held reset
  and lets jobs start again; `resets suspect "<name>" "<why>"` quarantines a job's command or a Wine
  test ("wine <file>:<test>"); `resets lift "<name>"` lifts one. Recording a cause resumes the
  groups, but not "default" and "hazard" while an alone job is queued or running. To see the hold without a reset:
  `ssh tom@192.168.2.183 powershell -ExecutionPolicy Bypass -Command ". C:\wc\rig\hazard.ps1;
  Set-Hold @('test')"`, then `resets record "test"`.
- `tools/graphics-rig/loop [picture-dir]`: one `gpu-job --desktop --build` that builds the rig on the desktop
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
  [--driver hardware|warp] [--level <level>] [<scene>...]`: one GPU job (on the hidden desktop) that builds the rig, copies gt-d3d11.exe and
  gt-d3d12.exe into C:\wc\out\<branch>\scenes\bin with each `--dll` beside them under its name
  (paths on the desktop, relative to its checkout; `%CARGO_TARGET_DIR%` works), runs every scene (or
  the named ones) in its own process with a 120 s limit, and checks every output against
  crates/wcontainers-graphics-test/references/<api>. Prints `pass`, `FAIL` or `absent` and the
  numbers for each output (absent: the scene program wrote `<scene>[-<part>].absent` saying what
  the device lacks, from the device's own answers; a note for a part with no reference is a FAIL), a FAIL for a scene that crashed, hung or has no references, for a scene whose program
  loaded d3d11.dll, d3d12.dll or dxgi.dll from anywhere but that directory when a replacement of
  that name was given (each program prints where it loaded all three from; a library it did not
  load is not held against it, and a replacement no scene loaded gets a note), and for a scene name
  that no scene program has; then whether the stored shader corpus matches the build, and a
  count of each verdict. Exits 1 on any FAIL; absent is not a failure. `--level` (Direct3D 11
  only) caps the feature level the programs ask for, so real Direct3D shows what a less capable
  device gets (numbers under "Ninth run"). Pointing it at our own Direct3D, both APIs in one command:
  `scenes --build "cargo build -p <crates>" --dll d3d11.dll=%CARGO_TARGET_DIR%\debug\<dll>
  --dll d3d12.dll=... --dll dxgi.dll=...`. Libraries of other names are only copied beside the
  programs. The scene programs do not ask for the debug layer when checking, so a replacement need
  not provide it.
- `tools/graphics-rig/scenes --record`: runs every scene with the debug layer on the GPU twice and on
  WARP; fails if the two GPU runs differ anywhere or the debug layer reports an error; then copies
  the GPU's outputs and references/<api>/tolerances.txt (each picture's allowance and how far WARP
  was) and the compiled shaders into this checkout, to be committed. Prints WARP's difference for
  every output. Run it after changing or adding a scene, look at the new pictures, commit.
- How a picture passes: it is compared at its own depth (8-bit, 16-bit or float) with the GPU's;
  with L the largest channel difference WARP had from the GPU, the tolerance is L capped at 8 steps
  of an 8-bit channel (2056 of a 16-bit one, 1/128 of a float one), and the pixels allowed beyond it
  are counted in each 16 by 16 tile: a tile may have as many as WARP had beyond it in that tile, so
  a replacement passes when it is at least as close to the GPU as WARP is, everywhere in the
  picture (WARP's freedom in one cell allows nothing in another). tolerances.txt lists each
  picture's tolerance, its total allowance, WARP's raw numbers and the tiles that allow any
  pixels (`tiles <index>:<pixels>`, index row by row). A Direct3D 12 picture the GPU draws exactly
  as its Direct3D 11 twin (same output name) takes the allowance of Direct3D 11's WARP picture
  when that is closer (fewer pixels beyond, then a lower tolerance), marked `d3d11` in its line:
  WARP draws the Direct3D 12 tessellation scene's isolines in wrong colours. A failing picture's line names how many
  tiles went beyond their allowance and the worst; "(allowed n)" is the total allowance, from
  WARP for scenes and from repeated runs for programs. A program's frame may also have `stray`
  pixels (from its repeated runs, see `programs run --record`) beyond their tiles' allowance
  anywhere in it. An output a scene or program writes that has
  no reference (a new part, a misnamed file) is a FAIL too. Texts (`.txt`: query counts,
  read-back bytes) must match exactly; capability answers (`.caps`: feature levels given for each
  request, format support) are compared word by word, where a lower answer (a level, a number,
  false or refused below true or a level) makes the output absent and a higher one or any other
  change fails it. Both hold only facts every correct implementation
  shares (timestamps as "in order or disjoint"; pixel shader invocations as "at least the covered
  pixels").
- Scenes are listed by `gt-d3d11 list` and `gt-d3d12 list`; a scene alone, on the desktop:
  `gt-d3d11 <scene>... <hardware|warp> <dir> [--debug] [--level <level>]` writes
  `<scene>[-<part>].<png|exr|txt|caps|absent>`; with --debug, a failing call's debug layer
  messages are printed with the error. The lowest level of each Direct3D 11 scene is in
  src/d3d11/scenes/mod.rs (SCENES); parts needing more check `Gpu::lacks_level`,
  `lacks_samples` or the device's options and write an absent note.
  The scene sources are crates/wcontainers-graphics-test/src/d3d11/scenes/*.rs and
  src/d3d12/*.rs; each file's first comment says what it covers. src/d3d12/states.rs,
  textures.rs, raster.rs, primitives.rs and draws.rs draw Direct3D 11's blend, depth, stencil,
  formats, samplers, textures, raster, msaa, targets, topology, geometry, tessellation, streamout
  and draws scenes (the same cells, from the same tables and lists of steps: the stencil scene's
  draws are `stencil::steps()`) through Direct3D 12, each state combination made into its
  own pipeline state object (states.rs's `Immediate`; streamout builds its two directly).
- Shaders: crates/wcontainers-graphics-test/shaders/*.hlsl, each naming its entry points in header
  lines `// fxc: <target> <entry>, ...` (DXBC for Direct3D 11) and `// dxc: ...` (DXIL, shader model
  6.0, for Direct3D 12). build.rs compiles them with the newest Windows SDK's fxc.exe and dxc.exe
  (10.0.26100.0; dxc 1.9.2602.17) when building for Windows; the programs embed the bytecode. Each
  Direct3D 11 entry is compiled at the lowest model fxc takes it at (4_0 for 50, 4_1 for 5 that
  read samples or gather or use cube arrays, 5_0 for compute, tessellation, conservative depth and
  the instanced geometry shader: 22, 4_0_level_9_1 for the levels scene: 2), so the scenes run on 10_x
  devices; the pictures were byte-identical to the shader model 5 ones. The
  corpus for the shader track is shaders/dxbc (79 files) and shaders/dxil (72 files), written by
  `scenes --record`; `scenes` says when it no longer matches the build.
- `tools/graphics-rig/wine-tests build`: fetches llvm-mingw 20261104 (its download checked against
  the SHA-256 GitHub lists for the release asset) and the Direct3D test sources
  of Wine 11.22 (commit 9f8c4ae4d35c7ff4153267aba9618c026f5f5583, sparse: the four test
  directories, libs/winecrt0, include/wine, include/ddk) into C:\wc\wine and builds
  C:\wc\wine\build\<file>_test.exe for d3d11, d3d10core, dxgi and d3d12 (desktop/wine-tests.ps1).
  Each statement directly in a file's START_TEST body that names a test_ function is rewritten in
  the copy built (C:\wc\wine\build\<file>.c) to run only when wc_wanted("<name>") says so
  (desktop/wine-select.h: the names in WC_WINE_ONLY, or all); the names go to <file>.tests. Not a
  GPU job. About 14 s with the sources present.
  It also fetches vkd3d 2.1 (commit fb7f9ff3c1f8d972ee6f1b8c6a92caf6c8ba1e10, sparse: tests,
  include, crosslibs) into C:\wc\wine\vkd3d and builds, as vkd3d's "crosstest" make target does,
  vkd3d-d3d12 (176 tests), vkd3d-d3d12_invalid_usage (3), vkd3d-hlsl_d3d12 (9) and
  vkd3d-shader_runner (302 .shader_test files, each a test named by its path under vkd3d's tests with
  + for /, for example hlsl+abs.shader_test). The headers vkd3d's build makes from .idl files come
  from llvm-mingw's widl; config.h is empty; the copy of vkd3d_test.h built picks a test by exact
  name (VKD3D_TEST_FILTER) instead of by substring. About 25 s for everything.
- `tools/graphics-rig/wine-tests list <file>`: the names of a file's tests.
- `gt-launch <report> <library>,...|- <program> [<argument>...]` (crate wcontainers-graphics-test):
  runs a program as the debugger of it and of every process it starts (Godot's console executable
  starts the real Godot), lets each process go once it has loaded every named library, and writes
  each place a process first loaded each library from, one line each ("not loaded" if none; a
  later load of the same name in a process is by full path, a stand-in loading what it stands in
  for, and is not counted). Exceptions go to the programs as without a debugger and the debug heap
  stays off. `wine-tests run` and `programs run` start every test and program through it, so a
  replacement Windows passed over (a wrong name, a program that limits its search) is a FAIL
  rather than real Direct3D's result under the replacement's name.
- `tools/graphics-rig/wine-tests run [--warp] [--record] [--dll <name>=<path>]...
  [<file>[:<test>/<test>...]]...`: one GPU job on the hidden desktop (desktop/wine-run.ps1) running
  every test of the named files (all eight by default), or only the named tests, each in its own
  process with a 120 s limit, on the GPU or on WARP, with replacement libraries beside the test
  programs if given (and the Windows SDK's dxcompiler.dll and dxil.dll, for vkd3d's shader model 6
  tests). Tests listed in tools/graphics-rig/wine-excluded.txt never run. Each test
  gets a verdict (pass, fail, crash, timeout) from Wine's summary line and exit code, compared with
  tools/graphics-rig/wine-baseline/<file>-<driver>.txt: "pass" (no failure, and at least as many
  checks run and no more skipped as on real Direct3D), "FAIL, passes on real Direct3D", "FAIL, ran
  <n> checks (real Direct3D <m>) and skipped <k> (real Direct3D <j>)", "FAIL, did not load the
  replacement" (gt-launch saw it load one from elsewhere), or "known" for tests that fail
  differently from run to run on the real thing (not counted; see below). Each file ends with how many of its tests
  pass, FAIL and are known by these verdicts; with replacements the run ends with how many tests
  loaded each from the test directory. The job builds the rig first (for gt-launch).
  Exits 1 on any FAIL. `--record` runs real Direct3D and copies the results into wine-baseline.
  Logs: C:\wc\out\<branch>\wine\logs, copied to the scratch directory (wine-logs/<branch>, emptied
  first, as other tracks' checkouts run the same command). Example:
  `wine-tests run d3d11:test_blend/test_texture` or
  `wine-tests run vkd3d-shader_runner:hlsl+abs.shader_test`. On the GPU a test quarantined on the
  desktop is not run, and a display driver reset during a test quarantines it and stops the run.
  `--record` of chosen tests replaces only their lines in the baseline. A test failing checks on real
  Direct3D is held to every other check, evaluation by evaluation: it runs with every check
  reported (WINETEST_REPORT_SUCCESS=1, VKD3D_TEST_DEBUG=2), so the n-th time a place in the source
  (file:line) is checked is known; the baseline's <file>-<driver>.failures lists, for each place,
  the evaluations real Direct3D failed (`test_x d3d11.c:17048 8-9,11-13,24-44`), and the test
  passes only if it fails no other evaluation ("FAIL, n checks fail that real Direct3D passes, the
  first (d3d11.c:17049#8): <its line>"), runs as many checks (for a test that crashed on real
  Direct3D, the checks it reported before the crash), skips no more and does not crash where real
  Direct3D did not. `--record` runs each test that does not pass twice more with every check
  reported; one whose failed evaluations or number of checks differ between the two is marked
  unstable in the baseline and only that kind is "known" (not counted). A whole run of all eight files takes
  about 9 minutes on the GPU, most of it the shader runner (about 1 s a file). vkd3d's tests give
  their verdict by vkd3d's summary line; on Windows its "todo" marks count as plain checks.
- `tools/graphics-rig/programs build`: fetches the real programs at pinned commits (or downloads
  released builds at pinned SHA-256 hashes into C:\wc\downloads) into C:\wc\programs\src, builds
  them with the Build Tools' MSBuild and has Godot import its demo projects (headless, no drawing)
  (desktop/programs.ps1, which lists them). About 6 minutes from nothing, most of it bgfx. The
  programs (25): sdk-tutorial02, 04 to 07; atg-simplebezierpc (tessellation), atg-simplecomputepc
  (compute), atg-simpleinstancingpc, atg-simplelightingpc and atg-simplemsaa_pc (Microsoft's
  Xbox-ATG-Samples at ff629a6e, PC samples drawing with the DirectX Tool Kit of the same
  repository, built with this Visual Studio's toolset and SDK, run in the repository's root, where
  they find their media); bgfx-01-cubes, 24-nbody, 26-occlusion (occlusion queries),
  35-dynamic (dynamic buffers), 37-gpudrivenrendering (compute and indirect draws) and 38-bloom
  (render targets of a downsampling chain): the bgfx examples that load no meshes, textures or
  fonts of their own (see Licences), fetched without those directories, on Direct3D 11 (`--d3d11`),
  and bgfx-01-cubes-d3d12 and bgfx-38-bloom-d3d12, the same two on bgfx's Direct3D 12 renderer;
  d3d12-hello-triangle and d3d12-hello-constbuffers (D3D12HelloTriangle and D3D12HelloConstBuffers
  of Microsoft's DirectX-Graphics-Samples at 68a2b1e9: a still triangle, and one a constant buffer
  moves a fixed step each frame; their projects import the Agility SDK 1.618.3 and DXC 1.8.2505.32
  NuGet packages, which programs.ps1 downloads from nuget.org at pinned SHA-256 and unpacks where
  the projects look for them, without NuGet itself, which writes into the user's profile); chocolate-doom (Chocolate Doom 3.1.1, `-timedemo demo1` of Freedoom 0.13.0's
  freedoom1.wad, one game tic a frame, window, no mouse, sound or music, SDL_RENDER_DRIVER=direct3d11);
  godot-platformer-3d-d3d11 and godot-platformer-2d-d3d11 (Godot 4.7.2's compatibility renderer on
  ANGLE's Direct3D 11, `--fixed-fps 60`); godot-platformer-3d-d3d12 and godot-platformer-2d-d3d12
  (Godot's Forward+ renderer on Direct3D 12). Nobody plays the games: Doom replays its demo, Godot's demos stand idle.
  Godot falls back to another renderer when the one asked for fails (Direct3D 12 to Vulkan, then
  to OpenGL; ANGLE to native OpenGL), so each run first writes the project's override.cfg (read by
  Godot after project.godot) turning every fallback off: a failing Direct3D then fails the program
  (Godot shows a message box nobody sees and the run ends at the 120 s limit).
- `tools/graphics-rig/programs run [--census] [--record] [--capture <frames>] [--dll <name>=<path>]...
  [--runs <n>] [<program>...]`: one GPU job on the hidden desktop running each program (copied to
  C:\wc\out\<branch>\programs\<program>\bin, in its own working directory, started through
  gt-launch) with the forwarding library as the library of its API (d3d11.dll or d3d12.dll),
  which takes the census, saves the frames listed (default 10,60,120), ends the program at the
  last, and gives it a clock that moves only by frames and sleeps (clock.rs, below). A replacement
  of that library given with --dll sits behind the forwarding one (WC_D3D11, WC_D3D12); other
  --dll libraries (dxgi.dll, the other API's library) go beside the program. A program fails unless some process of
  it loaded the library of its API (d3d11.dll, d3d12.dll) and dxgi.dll, each library placed beside
  it (the forwarding library, replacements) only from there, as gt-launch reports; and a Direct3D
  11 program fails when the implementation gave an error for a timestamp query (see the frame
  clock: the forwarding library asks the implementation until it answers, and an implementation
  that has not answered within 5 s fails the program too). A program whose frames are all unrepeatable says "no frame compared" instead of "every
  compared frame as on real Direct3D". Each program that ran to its last frame then has its frames compared with
  its references (`gt-run frames`, the scenes' rule with the tolerance at the cap of 8 steps and,
  per tile, only the pixels its own repeated runs on real Direct3D changed): prints `pass` or
  `FAIL` per frame and per program and exits 1 on any FAIL. The references are on the desktop
  (C:\wc\programs\reference\<program>: the frames and tolerances.txt; a copy in the scratch
  directory, program-references/<branch>), since the frames show the programs' own art under their own
  licences; tools/graphics-rig/program-references/<program> in the repository holds
  tolerances.txt and frames.sha256, and a program whose desktop references do not have those
  hashes fails ("run programs run --record"). `--record` (real Direct3D only) runs each program
  `--runs` times (default 10), each run in its own GPU job (the last job records), keeps the first
  run's frames, writes the tiles the other runs changed as allowed and leaves out the tiles meeting
  a program's `ignore` rectangle (bgfx's GPU memory readout, see programs.ps1); as `stray` it
  writes the most pixels one run had beyond the tolerance outside the ignored tiles (a few edge
  pixels of a multisampled mesh change on real Direct3D in tiles that move from job to job; these
  may fall anywhere), and prints after each run how many tiles
  the runs so far changed (to see whether more runs would find more); when a frame had more than a twentieth
  of its tiles changed, the program drifts, and all its frames are written "unrepeatable" and
  reported as "not compared". It copies tolerances.txt and frames.sha256 into the checkout, to be
  committed. Each run's outputs on the desktop replace the last run's, and the copies of the
  programs are deleted after each program ends, so only the frames, census, logs and settings come
  back. Frames and
  census go to the scratch directory (programs/<branch>/<program>, emptied first, as other tracks'
  checkouts run the same command at the same time); `--census` (real Direct3D only) also
  copies each census into tools/graphics-rig/census/<program>.txt. Every program
  gets APPDATA and LOCALAPPDATA in its output directory, so Godot's settings, logs and shader
  caches stay under C:\wc. The whole directory of each program's executable is copied (SDL and
  Godot's console wrapper load files beside it).
- The frame clock (crates/wcontainers-graphics-test-forward/src/clock.rs), on when WC_CAPTURE is
  set, started in the forwarding library's DllMain: it starts at the next whole second of the real
  clocks and moves by 1/60 s at each presented frame and by the time asked at each Sleep or
  SleepEx (which still sleep for real). ntdll's RtlQueryPerformanceCounter, GetTickCount,
  GetTickCount64, GetSystemTimeAsFileTime, GetSystemTimePreciseAsFileTime and winmm's timeGetTime
  (if loaded then) are detoured to it with the retour crate, so every caller in the process sees it.
  A program that waits for the clock to pass a time without sleeping or presenting would wait
  forever (the 120 s limit then fails it); none of the 25 does. While it runs, the forwarding
  library also answers Direct3D 11 timestamp and timestamp-disjoint queries at once from it (10
  MHz ticks, never disjoint), so GPU timings and query latency a program prints are fixed too; it
  still asks the implementation each time, again (flushing) until it answers, so a replacement's
  queries run and must complete, and reports the first error it gives or a query it has not
  answered within 5 s (real time, from QueryUnbiasedInterruptTime, which the clock does not
  replace), which fails the program. The answers' values are not checked there; the queries
  scene checks those. bgfx is built with
  BGFX_CONFIG_MULTITHREADED=0 (`programs build`, through MSVC's CL variable), so its examples draw
  on the thread that reads the clock and their frames follow it exactly.
- The forwarding library also stands in for d3d12.dll (copy it under that name): `scenes --api d3d12
  --dll d3d12.dll=%CARGO_TARGET_DIR%\debug\wcontainers_graphics_test_forward.dll` runs the Direct3D 12
  scenes through it; with WC_OUT it counts a Direct3D 12 program's device, command queue, command
  list, DXGI factory and swap chain calls, saves the frames WC_CAPTURE lists (the current back
  buffer copied on the queue the swap chain presents on, after the program's work) and WC_D3D12
  names a replacement behind it. Command lists: on each new device the library makes one list of
  each kind (direct, bundle, compute, copy) and patches their tables (four distinct tables here)
  before the program records any. With the Agility SDK (Microsoft's samples) the display driver
  gives each list, at its first Reset, a copy of that patched table with its own functions for
  the draws and the state it sets (DrawInstanced, IASet*, RSSet*, SetGraphicsRoot*,
  SetPipelineState and the like) and the patched entries for the rest; Reset and Close are typed
  hooks, so the copy is patched as soon as the first Reset returns, each entry going to the
  function it calls in the end, so a call is counted once. A list executed with a table that is
  neither is counted as a number in census.txt ("executed with a table not counted then").
  Reset, Close and ExecuteCommandLists also note calls made from Windows' own libraries (the
  system directory): DXGI resets, closes and executes a list of its own on the program's queue at
  each present (119 of each in 120 frames, in every Direct3D 12 program run). They stay in the
  counts and are listed in census.txt's line "made by Windows' own libraries". Hooking
  CreateCommandList crashed Godot (Measurements) and is not needed: a list keeps the patched table
  until its first Reset.
- `programs run` also checks a program's census against tools/graphics-rig/census/<program>.expected
  where there is one: each line there, the calls the program's source makes ("<table> <method>
  <calls>"), must be in census.txt as it is, or the program fails ("census has ... where the
  program makes ..."). The two Microsoft samples have one, from their source; it holds through a
  replacement library too, as the forwarding library counts in front of it.
- The census (crates/wcontainers-graphics-test-forward/src/census.rs), on when WC_OUT names a
  directory: the typed D3D11CreateDevice and D3D11CreateDeviceAndSwapChain exports patch the method
  tables of the device, swap chain and DXGI factory with counting thunks (names read from the
  SDK's C headers at build time) and hand the program a wrapper for each immediate and deferred
  context, whose methods count and jump through the context's own table at that moment (the
  runtime rewrites entries of a context's table as it runs, which patching missed); the typed
  D3D12CreateDevice export patches the Direct3D 12 device, the DXGI factory class, and (through
  the device's typed hooks) each command queue, command list and swap chain table. census.txt:
  "# frames", what was counted, any table entries the runtime changed after patching, then
  "<table> <method> <calls>" and "<library>.dll <export> <calls>". WC_CAPTURE: frames to save,
  before their present, as frame-<n>.png (8-bit RGBA and BGRA and 10-bit back buffers; other
  formats are noted as not saved); calls made while saving are taken back out; test presents are
  not frames.
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
- Tests: `cargo test -p wcontainers-graphics-test -p wcontainers-graphics-test-forward` (13 unit tests:
  comparison at each depth, NaN, counting per tile, the WARP allowance per tile, the allowance from
  repeated runs, stray pixels and how they are counted, outputs without references, absent notes and lower capability
  answers) on the Mac or through `remote`; clippy with `-D warnings` on both machines,
  since most of the code is Windows-only; `shellcheck -x tools/graphics-rig/*` (minus the .cmd) for
  the Mac scripts.
- The Mac scripts share tools/graphics-rig/common.sh: every ssh and scp to the desktop goes through one
  shared connection per branch (The owner's PC, above), so a run started while the desktop refuses
  new connections waits, backing off, instead of failing or retrying fast.
- `tools/graphics-rig/setup-desktop` also registers the failed-logon task (desktop/install-logons.ps1)
  and runs desktop/install-chrome.ps1 with the gt-chrome it builds. Chrome alone, into any folder:
  `remote "cargo build -q -p wcontainers-graphics-test --bin gt-chrome && powershell -NoProfile
  -ExecutionPolicy Bypass -File tools\graphics-rig\desktop\install-chrome.ps1 -Root <folder> -Wrapper
  %CARGO_TARGET_DIR%\debug\gt-chrome.exe"`.

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
- pueue 4.0.4 (pueue.exe, pueued.exe in C:\wc\bin), MIT OR Apache-2.0. Groups: default (4 at once),
  hazard (1 at once) and alone (1, with nothing beside it). The scheduled task wc-gpu-queue starts
  `pueued -c C:\wc\pueue\pueue.yml` at boot as tom (logon type S4U, so in session 0, without a window,
  whether or not anyone is logged in). Queue state and logs are in C:\wc\pueue.
- Chrome for Testing 157.0.8103.78 in C:\wc\chrome (0.43 GB), pinned, downloaded by
  @puppeteer/browsers (desktop/install-chrome.ps1); draw.mjs refuses to run if C:\wc\chrome holds any other build.
  Not installed into Windows: no updater, no services, no shortcuts. Never signed in. Since
  2026-11-17 its chrome.exe is the wrapper gt-chrome (0.2 MB) and the browser runs from
  chrome-win64-real, a tree of hard links (no extra space); chrome.exe.real is the browser.
- Since R2: the scheduled task wc-failed-logons (event trigger on Security events 4625 and 4740,
  runs desktop/logon-record.ps1 as tom with full rights, logon type S4U, at most 1 minute a run),
  and C:\wc\logs (jobs.log, display-resets.log, failed-logons.log, quarantine.txt, held.txt while
  held; a few KB). C:\wc\sshdiag (made by the orchestrator, not by this track) holds the wrapper's
  first build and notes.
- Since R2's second run: Microsoft's DirectX-Graphics-Samples (D3D12HelloWorld only) in
  C:\wc\programs\src\directx-graphics-samples (0.54 GB with the two samples' builds and the
  unpacked NuGet packages, 0.28 GB of it), the two packages in C:\wc\downloads (34 MB and 48 MB;
  downloads now 0.49 GB, C:\wc\programs 2.59 GB). A first build through MSBuild's NuGet restore
  created C:\Users\tom\AppData\Roaming\NuGet\NuGet.Config (210 bytes; that folder did not exist
  before) and C:\Users\tom\AppData\Local\NuGet\Migrations\1 (empty), at 22:47:05, the only
  contents of both NuGet folders; both folders were deleted minutes later, with C:\wc\nuget, and programs.ps1 no longer runs NuGet. 438.3 GB
  free on C: (other tracks build there too).

- Since R1's second run: C:\wc\bin\gt-desktop.exe (0.2 MB, built from this checkout by
  `setup-desktop`) and the scheduled task wc-desktop (runs it as tom with an interactive logon,
  limited rights, any number at once, no trigger: only started by jobs). It creates the desktop
  wc-rig in the owner's session while a job runs; nothing is shown on his screen. Requests pass
  through C:\wc\desktop. The real programs' sources and builds are in C:\wc\programs (0.94 GB).

- Since R1's third run: Godot 4.7.2 (C:\wc\programs\src\godot, the official Windows build, 0.17
  GB unpacked), Chocolate Doom 3.1.1 with its SDL2 libraries, Freedoom 0.13.0 and Godot's demo
  projects (sparse), all in C:\wc\programs (1.41 GB in all now), their downloads in C:\wc\downloads
  (0.41 GB in all now), and vkd3d's test sources and builds in C:\wc\wine (0.17 GB in all with
  Wine's). Godot's settings, logs and shader caches go to each run's output directory (APPDATA and
  LOCALAPPDATA are set per run) and, for the import step, C:\wc\programs\godot-data; nothing was
  written to the owner's profile. Measured at the end of the third run: C:\wc 41.05 GB, of which
  C:\wc\target 36.45 GB, but only 5.74 GB of that is this branch's build: the other graphics tracks
  (direct3d-11, direct3d-12, shaders, webgl, webgpu and two check directories) now build there
  through the rig too. 474.8 GB free on C:.

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
- End of the first R1 run: the 6 unit tests pass on the Mac and through `remote`, clippy with
  warnings denied passes on both, shellcheck passes, `queue-check` passes all five rules (two runs),
  and `loop` passes (141 scene outputs, 3 through the forwarding library, 4 other renderers).
- `cargo tree --workspace -e all` lists no Wine crate or source: the tests live only in C:\wc\wine
  on the desktop, fetched by the script.
- Swap chains in session 0 (the queue's session): CreateSwapChainForHwnd on a never-shown window with
  the flip model and with the blit model, and CreateSwapChainForComposition, all fail with
  DXGI_ERROR_NOT_CURRENTLY_AVAILABLE (0x887A0022). The scene is kept out of the list (its source is
  in the scratch directory, swapchain-scene.rs).

R1 second run (2026-11-17; desktop CPU load 1 to 5 % before each run, owner logged in at the
console, Mac load average 25 to 60):
- Presenting on the hidden desktop: a desktop job runs in session 1 on desktop wc-rig while the
  input desktop stays Default (checked with GetThreadDesktop and OpenInputDesktop from inside a
  job). The D3D11 and D3D12 swap chain scenes create, present (success codes), resize and draw on
  the GPU and on WARP with the debug layer; every Present there returns DXGI_STATUS_OCCLUDED
  (0x087A0001), as for any window nobody sees. A desktop job of `exit 3` took 0.6 s.
- Scenes: `scenes --record` drew all 36 scenes; the existing 141 outputs were unchanged and the 10
  new swap chain outputs equal on the two GPU runs; WARP differs from the GPU by at most 1 step in
  them. `loop`: 16 s end to end, 151 scene outputs, 3 through the forwarding library, 4 other
  renderers, all passing.
- Wine's tests, baseline (whole recorded run 1 min 36 s on the GPU, 18 s on WARP; a second GPU run
  gave the same verdict and the same number of checks for every test):
  d3d11: 166 tests run (1 excluded), 162 pass on the GPU (508407 checks, 51.5 s), 165 on WARP
  (508210 checks, 9.0 s). d3d10core: 111 run, 110 pass on both (198038 checks on the GPU, 32.8 s;
  3.6 s on WARP). dxgi: 24 run (18 excluded), 20 pass on the GPU (6228 checks), 21 on WARP.
  d3d12 (Wine's own, 10 tests): 7 pass on both (698 checks). The GPU's slowness per test is device
  creation, about 0.3 s a test against 0.02 s on WARP.
- Failing on real Direct3D, so not in the bar, with the reason read from their failed checks:
  Present returns DXGI_STATUS_OCCLUDED on the hidden desktop where the test expects S_OK, on the
  GPU and WARP alike: d3d11 and d3d10core test_desktop_window, dxgi test_swapchain_backbuffer_index,
  test_frame_latency_event, test_swapchain_present_count, d3d12 test_swapchain_draw,
  test_swapchain_size_mismatch, test_swapchain_backbuffer_index. Only on the RTX 3080's driver:
  d3d11 test_clear_buffer_unordered_access_view (223 checks: a raw clear writes 0xffffffff where
  0xfefefefe or 0x80808080 is expected), test_clear_image_unordered_access_view (3 checks: 0xffff7fff
  for 0xffff8000), test_h264_decoder (1 check: the decoder exists where the test expects it not
  to), dxgi test_output (3 checks: this monitor's modes are not sorted by refresh rate).
- Real programs: all 11 pass (120 frames presented, frames 10, 60 and 120 saved) on real Direct3D
  and through the forwarding library given as the replacement d3d11.dll (sdk-tutorial07 and
  bgfx-21-deferred; the log shows D3D11CreateDevice in both copies). Each takes 0.3 s (tutorials)
  or 2.4 to 2.9 s (bgfx). Over two runs, frames are the same only for still pictures
  (sdk-tutorial02, and sdk-tutorial07 by chance); the others animate by the clock. The census is
  the same over two runs for the tutorials and bgfx-24-nbody; the other bgfx examples differ by up
  to 8 lines, by a few calls of the device's AddRef, Release, QueryInterface and GetPrivateData.
- The census of our own scenes (swapchain, deferred, triangle) counted Draw 11 times on the
  immediate context, as many as the scenes make, once contexts were wrapped; with the context's
  table patched instead it counted 2 (the runtime changes those entries). Saved frames equal the
  stored references exactly (frames 1 and 3 of the swap chain scene, flip and blit).
- End of the second R1 run: the 6 unit tests pass through `remote`, clippy with warnings denied
  passes for both crates on the desktop, shellcheck passes for every Mac script, `queue-check`
  passes all five rules, `loop` passes, and `cargo tree --workspace -e all` (2095 lines) names no
  Wine or vkd3d crate.
- Disk: C:\wc 9.27 GB in all (C:\wc\target 5.50, C:\wc\programs 0.94, C:\wc\wine 0.16), 503.9 GB
  free on C: (508.8 GB at the end of R0's third run).

R1 third run (2026-11-17; desktop CPU load 1 to 5 % before each run, owner logged in at the
console, Mac load average 38 to 51):
- Games: chocolate-doom (Freedoom's demo through SDL's Direct3D 11 renderer) and the four Godot
  programs pass, 120 frames each, 3 frames saved; through the forwarding library 2.2 to 3.3 s for
  Doom, 24 to 33 s for the Godot 3D platformer on ANGLE (its shaders compiled cold every run), 4.5
  to 5 s for the 2D one, 17 to 32 s for Godot on Direct3D 12 (its movie writer reads every frame
  back). Over two runs, Doom's and Godot's Direct3D 11 frames were identical byte for byte, and so
  were the 2D Godot game's Direct3D 12 frames; the 3D one on Direct3D 12 differed at frames 60 and
  120 in 69 and 79 of 746496 pixels, by 1. Census: same over two runs for Doom and the 3D Godot
  game; the 2D one differed in 3 lines (CreateBuffer 112 against 92, Map and Unmap 21206 against
  21186, VSSetConstantBuffers 1430 against 1424). Godot's ANGLE back buffer is R10G10B10A2, now
  saved as a 16-bit PNG (each 10-bit value widened by repeating its top bits). All 16 programs
  pass in one `programs run` (1 min 49 s for the job). (Later found wrong for Doom: in the fourth
  check, 1 run in 4 differed at frames 60 and 120; the frame clock of the fifth run fixed it.)
- vkd3d's tests, baseline (two GPU runs gave the same verdict for every test):
  vkd3d-d3d12: 176 tests, 174 pass on the GPU (2488098 checks, 45 s), 175 on WARP (2488043 checks,
  8 s). vkd3d-d3d12_invalid_usage: 3 of 3 on both (56 and 59 checks). vkd3d-hlsl_d3d12: 9 of 9 on
  both (25192 and 25193 checks). vkd3d-shader_runner: 302 files, 299 pass on the GPU (194308
  checks, 265 s), 280 on WARP (171730 checks, 124 s; WARP has no Direct3D 9 device, so that part is
  skipped there).
- Failing on real Direct3D, so not in the bar, with the reason read from the failed checks: on both,
  vkd3d-d3d12 test_copy_block_compressed_texture (creating its texture returns E_INVALIDARG on this
  Windows 10 Direct3D 12 runtime, then the test crashes); on the GPU only,
  test_update_descriptor_tables_after_root_signature_change (1 check: a texel reads 0 where
  0xff00ff00 is expected), shader runner hlsl+mesh (mesh shader pipelines are refused,
  E_INVALIDARG, by this runtime), hlsl+uav-rwbuffer (6 checks: creating shader model 6 states
  returns E_FAIL where success is expected) and hlsl+uav-rwtexture (10 checks: success where
  failure is expected); on WARP only, 9 shader runner files crash (access violations in WARP,
  among them wave operations, switch and 64-bit casts) and 11 fail (half and float rounding,
  gather-cmp, UAV formats and others; each log names the line).
- Direct3D 12 state scenes: blend, depth, stencil, formats and samplers, recorded twice on the GPU
  with the debug layer (no error) and once on WARP. Against the Direct3D 11 references of the same
  scenes, every Direct3D 12 picture is identical (blend 4, depth 4 pictures, stencil, formats 16
  pictures and the support text, samplers address, gather, compare and compare_zero), except
  samplers-filters: 202 of 65536 pixels differ by up to 2 (the mip-filtered cells). WARP against
  the GPU: at most 2 steps, but for samplers-filters (anisotropy, as on Direct3D 11). The debug
  layer warns, not errs, for each pipeline made without vertex input and for dual-source pixel
  shaders writing a second target the pipeline does not bind.
- A bug of the first draft, kept here as a trap for the fronts: a Direct3D 12 command list's reset
  sets the blend factor back to zero, so it must be set again on every list (the blend-alpha
  picture differed from Direct3D 11's in 20966 pixels until it was).
- Check from a fresh checkout (clone of phase-2-graphics-rig at 2433400 in the scratch directory):
  `scenes --api d3d11 triangle blend depth stencil formats samplers` 47 passed, 12 s;
  `scenes --api d3d12 triangle blend depth stencil formats heaps` 39 passed, 7 s;
  `wine-tests run d3d10core` 110 tests counted, 0 FAIL, 111 s; `scenes --build ... --dll
  d3d11.dll=<forwarding library> --api d3d11 triangle blend stencil` 7 passed (each program loaded
  d3d11.dll from its own directory), 12 s; `programs run --dll d3d11.dll=<forwarding library>
  chocolate-doom` pass, 120 frames. With the blend constant's alpha changed from 0.45 to 0.46 in
  that checkout, `scenes --api d3d12 blend` failed blend-alpha (206 pixels beyond 2, by up to 3) and
  passed the other three, exit 1. `cargo tree --workspace -e all` (2112 lines) names no Wine,
  vkd3d, Godot, Doom or Freedoom crate. Licences read at the source on 2026-11-17 (GitHub's licence
  detection and the files themselves): DirectX-SDK-Samples MIT, bgfx, bx and bimg BSD-2-Clause,
  godot and godot-demo-projects MIT, chocolate-doom GPL-2.0 (COPYING.md), Freedoom BSD-3-Clause
  (COPYING.adoc), vkd3d LGPL-2.1 (COPYING and LICENSE at the pinned commit).
- End of the third run: `loop` passes (37 s with the build), the 6 unit tests pass through
  `remote`, clippy with warnings denied passes for both crates on the desktop, shellcheck passes.

R1 fourth run (2026-11-17; desktop CPU load 1 to 5 % before each run, owner logged in at the
console, Mac load average 17 to 25):
- Direct3D 12 twins of 11 more Direct3D 11 scenes, drawn twice on the GPU with the debug layer
  (identical, no error) and once on WARP. Against the Direct3D 11 references, with gt-compare at
  tolerance 0, every one of their 42 Direct3D 12 outputs is identical, every pixel: textures (8,
  BC1 to BC7 included), raster (4, geometry shader viewports included), msaa (8, alpha to coverage
  included), targets (4), topology (strip cut values 0xFFFF and 0xFFFFFFFF in the pipeline),
  geometry (7), tessellation, streamout's picture and buffer, draws (ExecuteIndirect for both
  indirect draws), constants (2) and views (3: plane slices 0 and 1 of a depth buffer, typeless
  formats, subranges). Streamout's text: "enough: 4 primitives written, 4 needed, 384 bytes
  filled", "small: 1 primitives written, 4 needed, 96 bytes filled". The Direct3D 11 scenes whose
  data moved into shared functions gave byte-identical outputs on recording.
- WARP against the GPU on Direct3D 12: at most 2 steps for all of them but tessellation (1232 of
  32768 pixels differ, by up to 228: WARP's isoline colours, see Known issues; on D3D11 768 by up
  to 4), textures-bc (as D3D11) and msaa-alpha_to_coverage (as D3D11).
- Traps found while writing them, worth knowing for the Direct3D 12 front: a stream output
  buffer's filled size is 8 bytes (the debug layer refuses a 4-byte location), and Direct3D 12 has
  no DrawAuto: the filled size is read back and drawn by count.
- `scenes --record`: 56 s end to end (the job about 32 s); `loop`: 24 s end to end, 234 scene
  outputs (130 D3D11, 104 D3D12), 3 through the forwarding library, 4 other renderers, all pass.
- Check from a fresh clone of phase-2-graphics-rig at 4668458 (in the scratch directory):
  `scenes --api d3d12 textures raster msaa targets topology geometry tessellation streamout draws
  constants views` 42 passed (job 3.4 s); `scenes --api d3d11 triangle topology draws textures
  views msaa` 22 passed; `wine-tests run d3d12` 10 tests run, 7 counted, 0 FAIL (the three known
  swap chain failures as recorded), 5.6 s; `scenes --build "cargo build -q -p
  wcontainers-graphics-test-forward" --dll d3d11.dll=<forwarding library> --api d3d11 views draws
  textures` 12 passed; `programs run --dll d3d11.dll=<forwarding library> sdk-tutorial07` pass,
  120 frames. With the D3D12 topology scene's 16-bit strip cut value set to disabled in that
  clone, `scenes --api d3d12 topology` failed (345 pixels beyond 1, by up to 249), exit 1.
  `cargo tree --workspace -e all --target all` (2841 lines) names no Wine, vkd3d, Godot, Doom or
  Freedoom crate. No dependency or program was added in this run, so no new licence; the third
  run read every licence at its source on 2026-11-17.
- End of the fourth run: the 6 unit tests pass through `remote`, clippy with warnings denied
  passes for both crates on the desktop, shellcheck passes for every Mac script.
- SSH to the desktop refused new connections once more, for about 8 minutes (05:59 to 06:07
  Mac time), cutting off a `scenes --record` (exit 255); run again after, it passed.

Fifth run (2026-11-17; desktop CPU load 1 to 5 % before each run, owner logged in at the console,
other graphics tracks' GPU jobs running beside ours at times; Mac load average 37 to 45). It fixed
the fourth check's findings:
- Conformance bar: with a d3d11.dll whose 51 exports all return E_FAIL (built in the scratch
  directory, stub/make-stub.ps1, into C:\wc\out\stub), `wine-tests run --dll
  d3d11.dll=C:\wc\out\stub\d3d11.dll d3d11` now gives "162 tests counted, 162 FAIL" (each "ran 16
  checks (real Direct3D 166) and skipped 7 (real Direct3D 0)" and the like; it gave 1 FAIL
  before); with such a d3d12.dll (18 exports), `vkd3d-d3d12 d3d12` gives "181 tests counted, 181
  FAIL". On real Direct3D all eight files, all tests: 784 counted, 0 FAIL (7 min 0 s for the job),
  so the check and skip counts of the baseline held on a new run.
- Allowance per tile: the checker's breaks, made again in this checkout: MIN_MAG_MIP_LINEAR
  changed to MIN_MAG_LINEAR_MIP_POINT in the filter cells now fails samplers-filters on both APIs
  ("3718 of 65536 pixels beyond 8 (WARP 6074) ... 16 tiles of 16 by 16 beyond WARP's count, the
  worst at 240,64: 256 pixels, WARP 0"); BC4_SNORM read as BC4_UNORM fails textures-bc on both
  ("4 tiles ... beyond WARP's count, the worst at 80,48: 248 pixels, WARP 59"); the bias clamp set
  to 0 fails depth-bias_depth on both (2704 pixels, 16 tiles). WARP's own differences in
  samplers-filters were measured per 64 by 64 cell: 64 pixels beyond 8 in each of the four point
  cells (one row), 2056 and 2746 in the two anisotropic cells, 1016 in the MaxLOD-0 cell, none
  beyond 2 steps in the trilinear cell.
- Direct3D 12 depth scene: bias_depth and format_d16, d24s8, d32 and d32s8 added (depth and
  stencil planes read back and laid out as Direct3D 11 reads them); all five equal Direct3D 11's
  references exactly. The bias and format tables are now one, in the Direct3D 11 depth scene.
- Samplers: reduction filters (minimum and maximum of point, linear and anisotropic, new output
  samplers-reduction, with samplers-reduction_support.txt saying the device has them: true on the
  GPU and WARP, both APIs) and comparison filters with linear mips and anisotropy (compare and
  compare_zero now 32 cells). WARP: reduction differs in 226 of 32768 pixels, up to 179 (the
  anisotropic cells), the comparison cells by at most 1.
- queries-results.txt now says "in order or disjoint" for timestamps.
- `scenes` with `--dll d3d11.dll=<forwarding library> --dll dxgi.dll=C:\Windows\System32\dxgi.dll
  triangle blend heaps blnd` (no --api): 12 outputs pass on both APIs (d3d12 heaps printed
  "d3d11.dll from not loaded, d3d12.dll from C:\Windows\SYSTEM32\d3d12.dll, dxgi.dll from
  C:\wc\out\phase-2-graphics-rig\scenes\bin\dxgi.dll"), and "FAIL blnd: no such scene", exit 1.
- `scenes --record`: 243 outputs (132 D3D11, 111 D3D12; the fifth run wrote 245, 133 and 112,
  wrongly: its log and the stored references both hold 243), 31 s for the job; the outputs other than
  the new and changed ones were byte-identical to the stored ones.
- Real programs on the frame clock: all 16 run, and on three recording runs every frame of the
  tutorials, Doom, Godot (both APIs) and five bgfx examples was the same in every tile. The bgfx
  examples showed real GPU timings, GPU query latency and GPU memory in their overlay until
  timestamp queries were answered from the clock (until then 2 to 10 tiles a frame changed and
  later runs changed others: 10 of 16 programs passed a check). bgfx-24-nbody's simulation
  drifts on the GPU itself: 623 to 632 of 3600 tiles of frame 120 and 227 to 231 of frame 60
  changed between runs, and in one of four checks one pixel of frame 10 was 9 steps off, so all
  its frames are recorded as unrepeatable and not compared. In the fourth check, chocolate-doom differed at frames 60 and 120 in 1 run of 4; on the
  clock, its frames were identical in all 3 recording runs and the checks below.

Sixth run (2026-11-17; SSH to the desktop refused, reset after key exchange, from 12:41 to 13:52
Pacific time, tried every 2 minutes, so with the fifth run's refusals about 4 hours in all; when
it came back the desktop had not rebooted (up since 23:49 the night before) and its clock was
right; desktop CPU load 13 % after the runs, the owner at the console, other tracks' jobs beside
ours; Mac load average 23 to 38):
- After `cargo clean -p` of the two crates, `programs run --record bgfx-24-nbody`: 60, 292 and
  692 of 3600 tiles of frames 10, 60 and 120 changed between its three runs, so all are
  unrepeatable; references committed.
- `programs run`, twice: 16 of 16 programs pass both times (nbody's frames "not compared"); the
  jobs took 3 min 2 s and 1 min 54 s.
- `programs run --dll d3d11.dll=<forwarding library> --dll dxgi.dll=C:\Windows\System32\dxgi.dll
  sdk-tutorial07 chocolate-doom`: both pass, every frame with 0 pixels beyond 8; gt-launch wrote
  "dxgi.dll from C:\wc\out\phase-2-graphics-rig\programs\sdk-tutorial07\bin\dxgi.dll" (the
  replacement d3d11.dll is loaded by the forwarding library by its full path, which aborts the
  program if it cannot). `wine-tests run --dll dxgi.dll=C:\Windows\System32\dxgi.dll
  d3d11:test_device_interfaces`: pass, 166 checks as on real Direct3D, "replacement dxgi.dll: loaded
  from C:\wc\out\phase-2-graphics-rig\wine\bin by 1 of 1 tests run".
- Break: Freedoom's demo2 in place of demo1 (not committed): chocolate-doom FAIL at all three
  frames (61242, 278820 and 287502 of 480000 pixels beyond 8), exit 1.
- Found: the Mac copies of program frames and Wine logs went to one scratch directory for every
  checkout, and another track's run (branch phase-2-graphics-direct3d-11) overwrote ours within
  minutes; `--record` and `--census` copy from there into the repository. They now go to
  programs/<branch> and wine-logs/<branch>, emptied first.
- Whole suite: fmt check, clippy with warnings denied and the 9 unit tests through `remote`;
  shellcheck of the 9 Mac scripts; `scenes` 243 passed, 0 failed (27 s); `loop` passes (32 s);
  `wine-tests run`, all eight files on the GPU: 784 counted, 0 FAIL (8 min 2 s for the job).

Seventh run (2026-11-17; desktop reachable throughout, owner at the console, other tracks' jobs
beside ours; Mac load average 20 to 40). It fixed the sixth check's findings:
- Godot on Direct3D 12 with the stub d3d12.dll (C:\wc\out\stub, every export E_FAIL): before,
  it fell back to Vulkan and passed. With Vulkan's fallback off it fell back to OpenGL 3 and drew
  120 frames, failing only on the pictures (130364 of 1536000 pixels beyond 8 at frame 10); with
  every fallback off, `programs run --dll d3d12.dll=C:\wc\out\stub\d3d12.dll
  godot-platformer-3d-d3d12` gives "FAIL, still running after 120 s, 0 frames" (Godot's message
  box), exit 1. A normal run logs "D3D12 12_0 - Forward+" and passes, every frame 0 pixels beyond 8.
- gt-launch follows child processes: godot-platformer-2d-d3d11 with `--dll
  dxgi.dll=C:\Windows\System32\dxgi.dll` now reports "dxgi.dll from ...\godot-platformer-2d-d3d11\bin\dxgi.dll"
  (before: "not loaded"), as do godot-platformer-3d-d3d12 and sdk-tutorial04; all pass. Recording
  every load rather than the first per process made sdk-tutorial04 fail ("d3d11.dll loaded from
  C:\Windows\System32\d3d11.dll", the forwarding library's own load by full path), hence the
  first-load rule.
- Wine with the stub d3d11.dll, `d3d11:test_blend/test_sample_mask`: "d3d11 on hardware: 2 tests
  run, 0 pass, 2 FAIL, 0 known" (before: "2 passed"). The manual's example `wine-tests run
  d3d11:test_blend/test_texture`: both pass (40 and 2094 checks).
- Scenes with the stub d3d12.dll, `--api d3d12 triangle heaps`: both FAIL with the stub's error
  and no "no scene run loaded" note (the programs now print where their libraries came from when
  the device cannot be made).
- Census: the 28 files of recording reruns removed (14 left, now 19 with the samples); `--census`
  skips <program>-again and -third.
- DirectX Tool Kit samples: five of Microsoft's Xbox-ATG-Samples PC samples built (with the whole
  `programs build`, 1 min 10 s with everything else built), 120 frames each in 2.4 to 3.7 s; over
  three recording runs every tile repeated but for 1 to 3 tiles of atg-simplemsaa_pc's frames
  (left out of its comparison). All five pass again on real Direct3D and with `--dll
  d3d11.dll=<forwarding library> --dll dxgi.dll=C:\Windows\System32\dxgi.dll` (dxgi.dll from bin).
  Their census: 48 to 70 methods called.
- Timestamp queries are now also asked of the implementation: the six bgfx examples (the
  programs that use them) pass as before.
- Device scene: 25 answers, the same on the GPU and WARP; `scenes --record` wrote only its output
  new, every other of the 244 outputs byte-identical to the stored ones (job 33 s).
- Unit tests: 10 (a new one: an output without a reference fails).
- Found by the whole suite: d3d11 test_shared_resource crashed (exit 0xc0000008, an invalid handle
  closed) once gt-launch debugged it, as Windows raises that only under a debugger; gt-launch now
  continues past it, as Windows does without one, and does not debug at all when no library is
  named. Then `wine-tests run d3d11`: 162 counted, 0 FAIL (1 min 18 s job); with `--dll
  dxgi.dll=C:\Windows\System32\dxgi.dll d3d11 d3d10core dxgi d3d12` (every test debugged): 299
  counted, 0 FAIL, dxgi.dll loaded from the test directory by 311 of 311 tests.
- Whole suite: fmt, clippy with warnings denied and the 10 unit tests through `remote`; shellcheck
  of the 10 Mac scripts; `wine-tests run`, all eight files: 784 counted, 1 FAIL (the crash above,
  fixed and run again since), 8 min 9 s for the job; `programs run`: 21 of 21 pass (2 min 21 s
  for the job); `scenes`: 244 passed, 0 failed (17 s job); `loop` passes (44 s end to end).
- Check from a fresh clone of phase-2-graphics-rig at e025ae2 (scratch, fresh-run7): `scenes --api
  d3d11 device triangle blend levels` 8 passed; `wine-tests run dxgi` 20 counted, 0 FAIL; `scenes
  --build ... --dll d3d11.dll=<forwarding library> --api d3d11 device draws textures` 10 passed;
  `programs run --dll d3d11.dll=<forwarding library> atg-simplebezierpc godot-platformer-3d-d3d11`
  both pass. With the device scene's "9_3 then 11_0" list reversed in that clone, `scenes --api
  d3d11 device` failed device-results.txt ("line 10: wanted ... level 9_3, got ... level 11_0"),
  exit 1. `cargo tree --workspace -e all --target all` (2926 lines) names no Wine, vkd3d, Godot,
  Doom, Freedoom or Xbox-ATG-Samples crate.
- Disk: C:\wc\programs\src\xbox-atg-samples 0.78 GB with its builds, C:\wc\programs 2.21 GB
  in all, 389.0 GB free on C: (474.8 GB at the third run; other tracks' builds share C:\wc\target).

Eighth run (2026-11-17; desktop reachable throughout, owner at the console; Mac load average 23
to 25). It fixed the two findings the seventh run left open:
- Absent verdict: `scenes --api d3d11 --level 11_0` on the GPU: 133 pass, 0 absent (NVIDIA
  answers minimum and maximum filtering at 11_0 too). At 10_0 with every shader still at model
  5: 3 pass, 130 absent. After compiling each shader at the lowest model fxc takes and declaring
  each scene's and part's level: 11_1 133 pass; 10_1 119 pass, 14 absent; 10_0 103 pass, 30
  absent, on the GPU and on WARP; 9_3 3 pass (the levels scene), 130 absent; never a FAIL.
- What real Direct3D refused or did differently below 11_0, found with these runs (the debug
  layer's messages now come with a scene's error): indirect draw argument buffers need 11_0
  (CreateBuffer E_INVALIDARG; draws is 11_0); blending set per target needs 10_1 (CreateBlendState
  at 10_0; targets is 10_1); the standard sample patterns need 10_1 (CreateTexture2D; msaa is
  10_1); a copy of BC1 blocks into R32G32_UINT left the destination untouched at 10_0, without
  an error (bc_as_uint is 10_1); the eight fractional viewports drew 1326 of 32768 pixels
  differently, by up to 229, at 10_0 and 10_1 (viewports is 11_0; that 10_x rounds viewport
  corners is our reading, not checked further).
- `scenes --record`, twice (jobs 47 s and 51 s): every picture byte-identical to the stored
  ones, shader model 4 bytecode included; only the texts that became capability answers or rules
  changed (levels-results.caps, levels-rules.txt, formats-support.caps; the reduction support
  texts went, the reduction picture being absent when the device says so). 243 outputs.
- Direct3D 12 tessellation: its allowance now comes from Direct3D 11's WARP picture: tolerance 4,
  0 pixels beyond (before: 8, 464 pixels in 13 tiles). The GPU passes it; WARP's own Direct3D 12
  picture now fails it (464 pixels beyond 4 in 13 tiles). No other Direct3D 12 picture had a
  closer Direct3D 11 WARP.
- Whole suite: fmt, clippy with warnings denied and the 11 unit tests through `remote`; shellcheck
  of the Mac scripts; `scenes` 243 passed, 0 failed, 0 absent (18 s job); `loop` passes (31 s end
  to end); `programs run` 21 of 21 pass (2 min 24 s job); `wine-tests run`, all eight files: 784
  counted, 0 FAIL (7 min 49 s job).
- Check from a fresh clone of phase-2-graphics-rig at 3dcb003 (scratch, fresh-run8): `scenes
  --api d3d11 --level 10_0 --build "cargo build -q -p wcontainers-graphics-test-forward" --dll
  d3d11.dll=<forwarding library> triangle levels formats msaa textures`: 33 passed, 0 failed, 11
  absent (formats-support.caps absent: 2 lines lower than the GPU's at 10_0); with the cube array
  part's level changed from 10_1 to 10_0 in that clone, `scenes --api d3d11 --level 10_0 textures`
  failed (9 FAIL, exit 1: the scene stopped at the shader model 4.1 shader); `wine-tests run dxgi`
  20 counted, 0 FAIL; `programs run --dll d3d11.dll=<forwarding library> atg-simplelightingpc`
  passes; `cargo tree --workspace -e all --target all` (2926 lines) names no Wine, vkd3d, Godot,
  Doom or Xbox-ATG-Samples crate.

Ninth run (2026-11-17; desktop reachable throughout, owner at the console, other tracks' jobs
running beside ours; Mac load average 16 to 25). It fixed the seven findings of the eighth run's
check:
- atg-simplemsaa_pc on real Direct3D: the eighth run's "21 of 21 pass" was not backed (the check
  saw it fail 4 of 6 times). With its old three-run references, 4 of 8 separate jobs failed here
  too; recorded again over 20 runs in one job (no stray allowance yet), 3 of 8 separate jobs still
  failed, each in a tile the 20 runs had not changed (1 to 6 pixels, up to 12 steps). A guess that
  sleeps on other threads moved the frame clock was tested and wrong: the only such sleeps were
  Sleep(0) (14915 in atg-simplemsaa_pc, 1331078 in bgfx-01-cubes), so that change was taken
  back. Now `--record` runs 10 times (`--runs`) and allows the most pixels any run had beyond the
  tolerance as stray pixels anywhere: atg-simplemsaa_pc got stray 4, 9 and 9 (frames 10, 120, 60)
  and 3, 6 and 3 changed tiles, still growing at the tenth run (frame 10: 1 tile after 8 runs, 3
  after 9). Then 10 separate `programs run atg-simplemsaa_pc` jobs: 10 pass; the most pixels beyond
  their tiles' allowance were 0 (frame 10, of 4), 6 (frame 120, of 9) and 0 (frame 60, of 9).
- Every program recorded again (`programs run --record --census`, 10 runs, job 18 min 43 s): the
  tutorials, Tool Kit samples, Doom and Godot's four repeat exactly (0 tiles, 0 stray);
  bgfx-24-nbody is unrepeatable as before (702 tiles of 3600 at frame 120); the other bgfx examples
  change only their 60 ignored tiles, with up to 173 stray pixels inside them. Census files
  changed by a few calls in bgfx-01-cubes (UpdateSubresource 16528 to 16642) and
  godot-platformer-2d-d3d11 (CreateBuffer, Map, Unmap), as they did before between runs.
- bgfx: fetched again without its examples' assets (47 MB of packs); 06-bump, 09-hdr,
  16-shadowmaps and 21-deferred replaced by 26-occlusion, 35-dynamic, 37-gpudrivenrendering and
  38-bloom (each ran its 120 frames in about 2.5 s). Their references and census are new.
- Copying back: `programs run` of two programs took 17 s end to end; the whole `programs run`
  took 2 min 8 s end to end for a 1 min 42 s job (eighth run: 8 min 53 s for 1 min 50 s). The
  copy after the ten-run recording still took about 10 minutes, nearly all Godot's 120 movie frames
  per Direct3D 12 program (75 and 79 MB), which are now deleted once the three compared are taken.
- Feature levels: `scenes --api d3d11 --level 10_0`: 109 pass, 30 absent on the GPU and on WARP
  (eighth run 103 and 30: draws, the unblended targets and BC1 to BC5 now run at 10_0, the
  indirect draws and BC6H and BC7 are their own absent parts); at 10_1: 125 pass, 14 absent; no
  FAIL. `scenes --record` (job 31 s, 64 s end to end): every stored picture byte-identical but
  draws.png and textures-bc.exr, which lost their 11_0 cells; new: draws-indirect,
  textures-bc67 and targets-plain_rgba8, _rgba16f, _rgb10a2, _r32ui, for both APIs. WARP's
  differences: draws 272 pixels by 1, draws-indirect 63 by 1, textures-bc 7621 by up to 0.052
  (2310 beyond 1/128 allowed), textures-bc67 1024 by up to 0.0037, targets-plain_rgba8 9779 by 1,
  _rgb10a2 6605 by 1, the float and integer ones none.
- Stencil: with the fan's back-face DECR changed to DECR_SAT in the Direct3D 11 table only,
  `scenes stencil` failed all four outputs of both APIs (2351 pixels, up to 255); restored.
- Wine and vkd3d baselines recorded again with the checks each test fails (GPU job 6 min 54 s,
  WARP 2 min 46 s): on the GPU 17 tests fail some checks (d3d11 4, d3d10core 1, dxgi 4, d3d12 3,
  vkd3d-d3d12 2, shader runner 3), on WARP 31; each failed the same checks when run again, so none
  is unstable (dxgi test_swapchain_backbuffer_index too, which crashed once in the sixth run).
  `wine-tests run` then: 801 counted, 0 FAIL on the GPU (job 6 min 33 s) and on WARP (2 min 30
  s); before, 784 were counted. With one of test_clear_image_unordered_access_view's three
  allowed checks taken out of the baseline, it failed ("1 checks fail that real Direct3D passes
  ... d3d11.c:17277") while test_clear_buffer_unordered_access_view passed with its 223.
- Whole suite: fmt, clippy with warnings denied and the 12 unit tests through `remote`;
  shellcheck of the Mac scripts; `scenes` 255 passed, 0 failed, 0 absent (14 s job); `scenes
  --driver warp` 254 passed and the Direct3D 12 tessellation FAIL that is WARP's own (Known
  issues); `loop` passes (23 s end to end); `programs run` 21 of 21 pass; `wine-tests run` as
  above. `cargo tree --workspace -e all --target all` (2929 lines) names no Wine, vkd3d, Godot,
  Doom, bgfx or Xbox-ATG-Samples crate.
- Check from a fresh clone of phase-2-graphics-rig at 5408650 (scratch, fresh-run9): `scenes
  --api d3d11 --level 10_0 --build "cargo build -q -p wcontainers-graphics-test-forward" --dll
  d3d11.dll=<forwarding library> draws targets textures stencil`: 14 passed, 0 failed, 7 absent
  (draws-indirect and textures-bc67 need 11_0, the blended targets and the cube array 10_1);
  `wine-tests run d3d11`: 166 counted, 0 FAIL (job 60 s); `programs run --dll
  d3d11.dll=<forwarding library> atg-simplemsaa_pc bgfx-26-occlusion`: both pass.

Tenth run (2026-11-17, 19:49 to 21:30; desktop reachable throughout, owner at the console, other
tracks' jobs beside ours; Mac load average 16 to 18). It fixed the three findings of the ninth
run's check:
- Stray pixels (blocking): they were the total pixels any repeated run differed in, the bgfx GPU
  memory readout included, so bgfx-26-occlusion, 35-dynamic and 37-gpudrivenrendering allowed 52
  to 173 pixels anywhere. Tried first: each run held to the tiles the other runs changed (leave
  one out). bgfx then got 0, but atg-simplemsaa_pc too little: recorded over 10 runs in one job
  (stray 2, 2, 0), 8 of 8 separate jobs passed, over 20 runs in one job 8 of 10, and with each run
  its own job (new, `programs run --record` now does that) over 10 runs 11 of 12 and over 30 runs
  17 of 20, every failure 2 pixels by 9 to 11 steps in a tile no recording run had changed. Its
  unstable pixels move between tiles from job to job, so the rule now is the most pixels one run
  had beyond the tolerance outside the ignored tiles. atg-simplemsaa_pc over 30 runs, each its own
  job (4 min 10 s): stray 2, 11 and 11 (frames 10, 120, 60), changed tiles 4, 9 and 4, still
  growing after the twentieth run; then 20 of 20 separate jobs passed, using at most 2 stray
  pixels. Every other program recorded again over 10 runs, each its own job (16 min 30 s; about
  1 min 40 s a job): tutorials, Tool Kit samples, Doom and Godot's four repeat exactly; the bgfx
  examples change only their 60 ignored tiles (stray 0 now); bgfx-24-nbody unrepeatable as
  before (700 tiles at frame 120). `programs run`: 21 of 21 pass (job 1 min 55 s). The check's
  break, its frame-60 of bgfx-26-occlusion with a 12 by 12 magenta square at 640,400, against the
  new references with `gt-run frames`: FAIL ("1 tiles of 16 by 16 beyond their allowance, the
  worst at 640,400: 144 pixels, allowed 0"); its unpainted copy passes (174 pixels, all in the
  readout).
- Driver resets: each GPU job now reads the System log for Display and nvlddmkm warnings and
  errors over its own time and fails with them (gpu-job). The query was checked on the check's
  break (19:35:32 to 19:35:38: nvlddmkm 153 and Display 4101 found; 19:36 to 19:40: none). Its
  first catch: the GPU Wine recording failed with a reset at 20:37:38, inside dxgi
  test_swapchain_backbuffer_index (log written 20:37:36.7 to 20:37:39.8; its Direct3D 12 part got
  0x887A0005 on real Direct3D), so that test is now excluded; the webgl track's job running beside
  it was failed too, as the rule says. No reset in any later job of this run.
- Wine checks in loops: a check is now its place in the source and which evaluation of it, and a
  test real Direct3D does not pass runs with every check reported, so a replacement must fail the
  very evaluations real Direct3D fails (test_clear_buffer_unordered_access_view: d3d11.c:17048
  evaluations 8-9,11-13,24-44,46-53, not any 34), and a test that crashed on real Direct3D records
  the checks run before the crash (vkd3d-d3d12 test_copy_block_compressed_texture 15 on the GPU and
  16 on WARP; the 11 shader runner files crashing on WARP 316 to 1881). Baselines recorded again
  (GPU job 8 min 2 s, WARP 4 min 0 s), none unstable; `wine-tests run`: 800 counted, 0 FAIL on
  the GPU (8 min 1 s) and on WARP (2 min 53 s); one fewer than before, the excluded test. Breaks
  on a recording of four tests: evaluation 8 of d3d11.c:17049 taken out of the allowed ones failed
  test_clear_buffer_unordered_access_view ("1 checks fail that real Direct3D passes, the first
  (d3d11.c:17049#8) ..."), and the crash test's recorded checks raised from 15 to 16 failed it
  ("ran 15 checks (real Direct3D 16)"); both passed again when put back.
- Whole suite: fmt, clippy with warnings denied and the 13 unit tests through `remote`, clippy
  and tests on the Mac; shellcheck of the Mac scripts; `scenes` 255 passed, 0 failed, 0 absent
  (15 s job); `loop` passes (18 s job); `cargo tree --workspace -e all --target all` (2929 lines)
  names no Wine, vkd3d, Godot, Doom, bgfx or Xbox-ATG-Samples crate.
- Check from a fresh clone at d7f8c38 (scratch, fresh-run10): `wine-tests run dxgi
  vkd3d-d3d12:test_copy_block_compressed_texture`: 24 counted, 0 FAIL; `programs run --dll
  d3d11.dll=<forwarding library> atg-simplemsaa_pc bgfx-26-occlusion`: both pass.

R2 first run (2026-11-17, from 21:30; desktop reachable throughout, owner at the console, other
tracks' jobs beside ours; Mac load average about 20):
- Carried findings. raster-scissor: the rectangle's bottom is now row 40, which cuts the fan (the
  fan reached only row 43 inside the old rectangle, so row 51 was never tested); recorded again on
  both APIs (every other output byte-identical); with the bottom at 39 instead, `scenes raster`
  fails raster-scissor on both APIs (5 pixels, up to 228, at 42,39). pueue's licence corrected to
  MIT OR Apache-2.0. dxgi test_swapchain_backbuffer_index: its cause is now established by the
  DRED report (The owner's PC); it runs on WARP (`wine-tests run --warp dxgi`: 24 counted, 0 FAIL)
  and is quarantined on the GPU.
- Resets traced from the System log (Display 4101, nvlddmkm 153), the Application log (WER
  D3DDRED2 reports, which name the process, and LiveKernelEvent 141), WER's archived reports (the
  path of the program for 20:37) and the checkouts' git reflogs; nothing was run on the GPU to find
  them.
- Quarantine: with `resets suspect "echo harmless r2 check" ...`, `gpu-job --reference -- "echo
  harmless r2 check"` was refused (exit 125, the reason printed); after `resets lift` it ran. With
  "wine dxgi:test_adapter_desc" quarantined, `wine-tests run dxgi:test_adapter_desc` printed "not run
  on the GPU, quarantined" and ran nothing; lifted after.
- Hold: with Set-Hold called by hand, the three groups were paused, `gpu-job` was refused (exit 125,
  "the queue is held ..."); `resets record` resumed them and the next job ran. Found doing this: pueue's
  `pause` without `--wait` suspends running jobs; the first hold test and two queue-check runs did that
  to other tracks' running jobs for a few seconds each, before it was changed to `--wait` everywhere.
- `queue-check`: all six rules pass (the new one: two hazard jobs ran one after the other).
- Failed logon: one network logon with a wrong password (LogonUser, made once on purpose at
  22:11:44) appeared in C:\wc\logs\failed-logons.log within seconds: "4625 account ...\tom, logon type
  3, status 0xc000006d 0xc000006a, logon process Advapi Negotiate, process ...\powershell.exe".
- Chrome: `setup-desktop` replaced the first wrapper build in C:\wc\chrome with gt-chrome ("put the
  wrapper in front of ...chrome.exe.real").

Direct3D 12 through the forwarding library (R2 first run; the library is one DLL mirroring the
exports of both d3d11.dll and d3d12.dll, copied under either name; see Loading a replacement):
- `scenes --api d3d12 --build "cargo build -q -p wcontainers-graphics-test-forward" --dll
  d3d12.dll=%CARGO_TARGET_DIR%\debug\wcontainers_graphics_test_forward.dll`: 116 passed, 0 failed (job
  8 s), each scene loading d3d12.dll from its own directory. `loop` (the Direct3D 11 forwarding run)
  and `programs run sdk-tutorial07 bgfx-01-cubes godot-platformer-2d-d3d12` pass as before.
- bgfx-01-cubes with `--d3d12` and the library as d3d12.dll (WC_OUT and WC_CAPTURE set; trial,
  not in the program list): 120 frames; census counted the factory (IDXGIFactory7), the device
  (ID3D12Device8, the newest this Windows 10 runtime answers), the queue, the command list
  (ID3D12GraphicsCommandList6) and the swap chain; no frame saved at first: "No such interface
  supported (0x80004002)": IDXGISwapChain::GetDevice does not answer with the queue. Now the library
  keeps the queue each swap chain was made on (from the factory's CreateSwapChain* calls) and copies
  the current back buffer on it: frames 10 and 60 of bgfx-01-cubes --d3d12 were saved (the overlay
  reads "Renderer: Direct3D 12 / NVIDIA"; not yet compared with references).
- Godot's 2D platformer on Direct3D 12 through it: runs with the library alone and with the
  census of device, queue, factory and swap chain; crashes (access violation in the system
  d3d12.dll at +0x122d7, inside d3d12core.dll) as soon as CreateCommandList and CreateCommandList1
  are hooked to patch the command lists' table. Ruled out by trials: the IUnknown entries, either
  half of the list's 80 entries, all of them (the crash stays with no entry patched), and the
  QueryInterface calls made to find the list's interfaces. So command lists are not counted yet,
  and census.txt says so.

R2 second run (2026-11-17, from 22:40; desktop reachable throughout, owner at the console, other
tracks' jobs beside ours; Mac load average 58 to 78):
- Command lists counted without hooking CreateCommandList: the library makes a list of each kind
  on each new device and patches their tables. Godot's 2D platformer on Direct3D 12 then ran its
  120 frames with the census on (job 7 s), counting 46055 SetGraphicsRootDescriptorTable, 19636
  DrawIndexedInstanced and the rest; no crash. Patching tables found at ExecuteCommandLists as
  well was tried first: in D3D12HelloTriangle it found two more tables, and the census showed
  ClearRenderTargetView 239, Close 476 and Reset 476 for 120 frames where the program does one
  clear, one Reset and one Close a frame: those lists are the runtime's own. Counting only the
  device's tables: ClearRenderTargetView 120, Reset 239, Close 240 and 238 lists "with a table
  not counted". That census left out most of the samples' own calls, every draw among them
  (found by R2's check; the third run below explains and fixes it).
- Microsoft's samples: `programs build` fetched DirectX-Graphics-Samples (sparse, D3D12HelloWorld)
  and built D3D12HelloTriangle and D3D12HelloConstBuffers with the Build Tools (MSBuild with
  -restore first: it worked but wrote into the owner's profile, see Departures; then from the
  packages unpacked by programs.ps1, rebuilt from nothing after deleting bin and obj). Both use
  the Agility SDK's D3D12Core.dll from their D3D12 folder: the census shows ID3D12Device14 and
  ID3D12GraphicsCommandList10, where the system runtime gives bgfx and Godot ID3D12Device8 and
  ID3D12GraphicsCommandList6.
- `programs run --record --census` of the six Direct3D 12 programs (bgfx-01-cubes-d3d12,
  bgfx-38-bloom-d3d12, d3d12-hello-triangle, d3d12-hello-constbuffers, godot-platformer-2d-d3d12,
  godot-platformer-3d-d3d12), 10 runs, each its own job (about 25 s a job, the last 61 s):
  the two samples and both Godot games drew every frame identically in all 10 runs (0 tiles
  changed); each bgfx example changed 62 to 64 of its 3600 tiles, all in the overlay's lines at
  rows 13 to 16 (x 0 to 319): its "Submit CPU, GPU" line shows GPU time from Direct3D 12 timestamp
  queries, which the frame clock does not fix (it answers only Direct3D 11's), and the GPU memory
  line, left out already. Godot's Direct3D 12 frames are now the swap chain's 960x540 (the movie
  writer's references had another size, so the old references failed until recorded again).
- Then `programs run` of the six with the forwarding library as d3d12.dll: all pass (bgfx-01-cubes-d3d12
  frame 60: 124 pixels beyond 8, all within their tiles' allowance).
- Whole suite at 2a5c1b7 plus the census commits (scratch r2b/suite): fmt, clippy with warnings
  denied and the 13 unit tests through `remote` and on the Mac; shellcheck of the 11 Mac scripts;
  `scenes` 255 passed, 0 failed, 0 absent (15 s job); `scenes --api d3d12 --dll d3d12.dll=<the
  forwarding library>` 116 passed (8 s job); `loop` passes (23 s job); `programs run` all 25 pass
  (bgfx-24-nbody run but not compared, as before; 2 min 0 s job); `wine-tests run` 800 counted, 0
  FAIL on the GPU (the quarantined dxgi test not run; job 9 min 59 s, after waiting 69 s behind
  another track's hazard job) and `wine-tests run --warp` 801 counted, 0 FAIL (3 min 8 s);
  `queue-check`: all six rules pass.
- Check from a fresh clone of the pushed branch at d5bb944 (scratch, fresh-r2b): `scenes blend
  stencil draws raster` 24 passed; `wine-tests run dxgi` 23 counted, 0 FAIL (the quarantined test
  not run on the GPU); `programs run --dll d3d12.dll=<the forwarding library>
  d3d12-hello-constbuffers bgfx-01-cubes-d3d12 godot-platformer-2d-d3d12` (a second copy behind
  the first, through WC_D3D12; its log shows each D3D12CreateDevice passing through both): all
  three pass.

R2 third run (2026-11-18, from 00:00; desktop reachable throughout, other tracks' jobs beside
ours; Mac load average 45 to 70):
- Why the samples' census missed their draws, found with a temporary dump of every table
  executed: both uncounted tables were heap copies of the patched D3D12Core.dll table (the direct
  list's), with 38 entries pointing into the NVIDIA driver (nvwgf2umx.dll: DrawInstanced,
  DrawIndexedInstanced, Dispatch, IASetPrimitiveTopology, IASetVertexBuffers, RSSetViewports,
  RSSetScissorRects, SetPipelineState, the SetGraphicsRoot* and SetComputeRoot* setters and
  others) and the rest pointing to our thunks. A second dump showed the switch happening at each
  list's first Reset (before it, the list had the patched table), and the call stack at that first
  Reset showed the second list a frame is DXGI's (dxgi.dll called its Reset), in every Direct3D 12
  program (bgfx and Godot as well), reset 119 times in 120 frames. bgfx and Godot, on the
  system's Direct3D 12 runtime, get no such copies.
- After the fix, `programs run --census` of the six Direct3D 12 programs: all pass. The samples
  now count DrawInstanced, IASetPrimitiveTopology, IASetVertexBuffers, RSSetViewports,
  RSSetScissorRects and SetGraphicsRootSignature 120 each (and SetGraphicsRootDescriptorTable 120 in
  ConstBuffers), Reset 239 and Close 240 of which DXGI's 119 each; the census line "made by
  Windows' own libraries: command-list Close 119, command-list Reset 119, command-queue
  ExecuteCommandLists 119". Every line of the two .expected files matched. With one line of
  d3d12-hello-triangle.expected changed to DrawInstanced 121, the run failed: "census has
  'command-list DrawInstanced 120' where the program makes 'command-list DrawInstanced 121'".
  Godot's 2D game with CreateCommandList and CreateCommandList1 hooked crashed again at once
  (0xc0000005, 0 frames), so they are not hooked; with Reset and Close hooked it ran its 120 frames.
- Three census runs of the six (one job each, about 30 s): the samples and bgfx-38-bloom-d3d12
  identical in all three; bgfx-01-cubes-d3d12 differed in 7 methods (CopyTextureRegion 691/689/689,
  ResourceBarrier 2111/2107/2107, CheckFeatureSupport 2477/2471/2471, CreateFence 692/690/690,
  GetCustomHeapProperties 19/18/18, AddRef and Release by 3); both Godot games in the device's
  AddRef and Release only (by up to 20). R2's check had seen bgfx-38-bloom-d3d12's CheckFeatureSupport go from
  2484 to 1797 and CreateFence from 692 to 463 between two runs.
- The queue: `queue-check` passes all six rules with the queue deciding the groups (third try:
  the first failed as a job's arguments may not hold spaces, which the rig key first had, so
  every rig run failed to start from 00:21:28 to 00:23:20; no other track's job was a rig run
  then; the second found an alone job let others start 0.3 s before it ended, now fixed by
  resuming the groups only after its reset check). One sleep job of a new rig tree ran in group
  hazard, the next in default. `resets suspect "echo r2c harmless suspect"`, then `gpu-job --
  "echo r2c harmless suspect&& rem with C:\wc\out\other\a"` and `remote "echo r2c harmless
  suspect"` were both refused (125) naming the quarantine; lifted after. A hold (Set-Hold by hand
  for about 3 s) refused both `gpu-job` and `remote`; `resets record` resumed the three groups.
  The quarantine matching was also checked on a scratch quarantine file (7 cases: the same
  command with another output directory, with words added before and after, matched; a shorter
  command, another test target and an unrelated build did not).
- Unit tests: 17 on Windows (13 as before, 3 for gt-chrome's argument handling, 1 for the census
  patching a copied table), the 16 that are not Windows-only on the Mac.
- bgfx-01-cubes-d3d12 and bgfx-38-bloom-d3d12 recorded again (10 runs each, a job each, about 7 s
  a job) with the GPU time line left out: 80 of 3600 tiles (the left-out rectangle) changed from
  run to run and no pixel outside it, so their stray allowance is 0, as for their Direct3D 11
  twins (it was 66 to 133).
- Whole suite at 8cf0f76 (scratch r2c/suite): fmt, clippy with warnings denied and the unit tests
  (17 through `remote`, 16 on the Mac); shellcheck of the 11 Mac scripts; `scenes` 255 passed, 0
  failed, 0 absent (15 s job, which ran in group hazard as the first of a new rig tree, after 78 s
  waiting behind other tracks' hazard jobs); `scenes --api d3d12 --dll d3d12.dll=<the forwarding
  library>` 116 passed (8 s); `loop` passes (23 s, in group default); `programs run` all 25 pass
  (bgfx-24-nbody run but not compared; 1 min 42 s); `wine-tests run --warp` 801 counted, 0 FAIL,
  dxgi test_swapchain_backbuffer_index run on WARP (2 min 41 s); `wine-tests run` 800 counted, 0
  FAIL, that test "excluded" on the GPU (8 min 38 s); `queue-check` all six rules pass.

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

Direct3D 12: build.rs mirrors d3d12.dll's exports too (ordinals 99
to 117, which d3d11.dll's 1 to 51 do not overlap; ordinal 99 has no name and is forwarded by
ordinal). D3D12DeviceRemovedExtendedData is data, which a thunk cannot forward, so the library
lacks it (census.txt says so); a program importing it would not load. A replacement d3d12.dll goes
behind it with WC_D3D12, as WC_D3D11 does for d3d11.dll. D3D12CreateDevice is typed: it patches the
device's table and the DXGI factory class's table (from a factory it makes, as Direct3D 12 devices
have no adapter to ask for the program's), and CreateCommandQueue(1) patch the queue's.

## Departures from the plan

There is no plan document in this checkout; these depart from the task or brief as written.
- winget could not be used: over SSH its source update fails (Appx deployment: "A specified logon
  session does not exist"). Each tool came from its official direct installer instead, with pinned
  versions in install-tools.ps1.
- Graphics Tools cannot be added from an SSH logon (Add-WindowsCapability and DISM: access denied),
  so a one-off scheduled task as SYSTEM adds it.
- Chrome is Chrome for Testing in C:\wc\chrome rather than Google Chrome installed into Windows: the
  same browser build, without an updater, services or entries in the owner's machine, and pinned.
  It is the build pinned in desktop/install-chrome.ps1; to move to another, change the pin there
  and run setup-desktop, which installs it with the wrapper in front. Installing Chrome any other
  way (for example `npx @puppeteer/browsers install` by hand) leaves out the wrapper, and
  draw.mjs refuses to run when more than one build is present.
- The job queue client and the remote command are shell scripts in tools/graphics-rig (with PowerShell
  halves for the desktop), not Rust in the crate: they only drive ssh, scp and pueue.
- The queue tool is pueue (MIT OR Apache-2.0): it runs on Windows, has groups with a parallel limit and per-job
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
- The scenes' reference pictures are kept in the repository (255 outputs of both APIs) so a change
  that affects every renderer alike is caught. The real programs' reference frames are not: they
  show the programs' own art (the fonts of bgfx's overlay, the Tool Kit samples' media, Freedoom,
  Godot's demo projects) under those projects' licences, so they stay on the desktop and in the scratch directory, and the repository
  pins them by SHA-256.
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
  fences, copies and resolves), and repeat Direct3D 11's scenes of drawing state (blend, depth,
  stencil, formats, samplers, textures, raster, msaa, targets, topology, geometry, tessellation,
  streamout, draws, constants, views). Not repeated: the Direct3D 11 scenes of calls Direct3D 12
  does not have (dynamic: Map with discard and no-overwrite and UpdateSubresource; mips:
  GenerateMips; deferred: deferred contexts; levels: feature levels 9_1 to 10_1), and quads and
  instanced, whose ground the draws scene covers.
- Direct3D 12 has no DrawAuto and no stream output overflow predicate: its streamout scene reads
  the buffer's filled size back and draws that many vertices, and its text gives the statistics
  and the filled size instead of an overflow flag.
- Wine's tests are built with llvm-mingw (clang and mingw-w64's headers and import libraries)
  rather than MSVC, each test file compiled alone in Wine's standalone test mode (STANDALONE,
  __WINESRC__), with Wine's include/wine headers, winecrt0's debug.c (the debug string helpers Wine's
  ntdll exports), and four Windows headers mingw-w64 lacks taken from Wine (ddk/d3dkmthk.h,
  d3dukmdt.h, dxcore.h, dxcore_interface.h); ARRAY_SIZE and DECLSPEC_EXPORT come from Wine's
  winnt.h, so they are defined on the command line, and an import library for d3d10_1.dll is made
  with llvm-dlltool. Wine's other headers are not used: half of them are generated from .idl files
  by Wine's build and clash with mingw-w64's.
- Jobs that present run on a hidden desktop in the owner's session (the earlier blocker, which
  the first run left for the owner): it keeps everything off his screen and uses only a scheduled
  task with an interactive logon, which changes nothing in Windows' settings. Windows of a desktop
  nobody sees are occluded, which costs the eight Wine tests and the program named in Known issues.
- A Wine or vkd3d test counts as passing only if it also runs at least as many checks, and skips
  no more, than on real Direct3D: a library that refuses to make a device skips nearly everything
  without a failure. A replacement that honestly reports a feature absent therefore shows as
  differing in the tests that skip for it; the line says by how much.
- Wine's tests run one test per process rather than one file per process, so a crash loses one
  test; Wine's framework has no way to pick a test, so the build rewrites START_TEST in its copy.
- Nineteen Wine tests are excluded: they would change the owner's display, found by searching
  each test's calls for ChangeDisplaySettings, SetFullscreenState, fullscreen swap chains,
  SetGammaControl and ClipCursor. dxgi test_swapchain_backbuffer_index, whose Direct3D 12 part hangs
  the RTX 3080, is excluded on the GPU only (marked "(gpu)" in wine-excluded.txt) and also
  quarantined on the desktop, so it runs on WARP and never on the GPU.
- Every job runs one at a time in the queue group hazard unless the queue sees it is one of the
  rig's own runs on rig code already seen to run without a reset: the task asks that kinds seen
  to reset the driver run one at a time; those kinds include the rig's own scene program after an
  edit (19:35) and, as suspects of the unknown 16:44, other tracks' own code; and a flag chosen by
  the sender was found used by other tracks for their own GPU code. Tracks that build inside their
  jobs hold that one slot while they build; `gpu-job --build` builds outside it. Measured load
  before the change (C:\wc\logs\jobs.log, 2026-11-17 21:58 to 2026-11-18 00:13): 294 jobs,
  hazard 108 of them for 102 minutes, default 174 for 25 minutes, alone 12 for 10 minutes; at
  most 5 at once.
- vkd3d's tests are built like Wine's, with llvm-mingw rather than vkd3d's own configure and
  make (no MSYS or autotools on the desktop): the headers vkd3d's build generates come from
  llvm-mingw's widl (run in the output directory, as widl names each header's guard after the path
  given), config.h is empty, and the one change to vkd3d's framework is that the copy of
  vkd3d_test.h compiled selects a test by exact name rather than by substring, so one test can run
  alone. The shader runner is included: its tests compile HLSL with the system's d3dcompiler_47.dll
  and the SDK's dxcompiler.dll and run them on Direct3D 9, 11 and 12, which a replacement library
  must also serve.
- The Godot games reach Direct3D 11 through ANGLE (Godot's compatibility renderer, as Chrome's
  WebGL does), not through a Direct3D 11 renderer of their own: no permissively licensed game with
  freely licensed data and a native 64-bit Direct3D 11 renderer was found. Chocolate Doom uses
  SDL 2's Direct3D 11 renderer, a native one. Chocolate Doom is GPL-2.0: it is downloaded as its
  released build and run for testing only, never part of the repository or any crate.
- Godot on Direct3D 12 now runs without its movie writer, saved by the forwarding library like
  every other program, on its frame clock. Microsoft's Direct3D 12 samples' NuGet packages are
  downloaded at pinned SHA-256 and unpacked by programs.ps1 instead of restored by nuget.exe or
  MSBuild's restore: tried once, MSBuild's restore wrote NuGet.Config into the owner's
  %APPDATA%\NuGet and a Migrations folder into %LOCALAPPDATA%\NuGet even with its own config file
  given (both removed at once, as was everything it put in C:\wc\nuget).
- Godot's renderer fallbacks are turned off by an override.cfg the rig writes into its copy of each
  demo project (C:\wc\programs\src\godot-demo-projects), not by changing the projects: it is
  Godot's own way to override project settings, and a fallback would draw through another API
  while the run is compared as Direct3D.
- Programs are made repeatable by the rig, not by changing them: the forwarding library detours
  the process's clock functions to a clock that moves by frames (rather than a fixed camera per
  program, which would be shaped around each one), and bgfx is built single-threaded with a
  compiler define (its render thread would read frames against a clock moved by another thread).
- A real program's frame has no WARP picture, so its allowance comes from ten runs on the GPU:
  tolerance 8 steps (the scenes' cap), the tiles the program draws differently from run to run
  are left out, and the most pixels one run had beyond the tolerance outside the ignored
  rectangle may fall anywhere else (`stray`): atg-simplemsaa_pc
  differs on real Direct3D in a few edge pixels whose tiles change from job to job, so no number
  of runs in one job finds them all (Ninth and tenth runs), and each run of a recording is its own
  job. tolerances.txt says how many.
- Only bgfx examples that load no meshes, textures or fonts of their own are run, and bgfx is
  fetched without its examples' asset directories: bgfx's docs/license.rst lists several of those
  assets (the Uffizi light probe, the Stanford bunny and others) without a licence, so they are not
  clearly licensed for this use. 06-bump, 09-hdr, 16-shadowmaps and 21-deferred were replaced by
  26-occlusion, 35-dynamic, 37-gpudrivenrendering and 38-bloom (19-oit was tried and dropped: on
  Direct3D 11 at this commit it draws only its overlay on white).
- The Direct3D 12 state scenes take their cells from the Direct3D 11 scenes' own tables (blend
  descriptions, comparison functions, depth biases and formats, formats, samplers) and convert
  each description by value
  (the enumerations have the same values), so both APIs are tested on the same ground and their
  pictures can be compared with each other as well as with WARP.
- The DirectX SDK samples built on DXUT are not used: DXUT needs Microsoft's D3DX NuGet package,
  whose licence must be accepted on download (requireLicenseAcceptance, Microsoft's own terms).
  Only the five plain Direct3D 11 tutorials are used, which need nothing but the Windows SDK.
- The census forwarding library hands the program wrapped contexts instead of patching their table
  (see Measurements); devices, swap chains and factories are patched, and any entry the runtime
  changes afterwards is reported in census.txt.
- A scene part a device lacks is reported absent, a third verdict that does not fail the run, and
  capability answers are compared as "at most the GPU's" (eighth run); the brief's rule is that what
  a back end cannot do is reported absent through the API's own capability mechanism, so the rig
  takes the device's answers as the authority on what to draw.
- The Direct3D 11 scenes' shaders are compiled at the lowest shader model that takes them rather
  than all at 5_0, so 24 of the 28 scenes run on the 10_0 device (22 in the eighth run) the plan gives the WebGL 2 route;
  the corpus for the shader track now holds 4_0, 4_1 and 5_0 bytecode.
- Texts hold only facts every correct implementation shares; values the implementation chooses
  (timestamps, helper pixel invocations, append order) are recorded as facts about them or sorted.

## Known issues

- WARP's Direct3D 12 isolines take other colours than the GPU's (the isoline cell of the D3D12
  tessellation picture); its Direct3D 11 ones do not, and the GPU's D3D12 picture equals its D3D11
  one exactly, so it is WARP's DXIL path. The D3D12 picture's allowance therefore comes from
  Direct3D 11's WARP picture (How a picture passes); WARP itself now fails that D3D12 picture.

- A scene or replacement that hangs the GPU makes the driver reset itself (TDR), which blanks the
  owner's screen for a moment; the job then fails, the queue is held and what ran is quarantined
  (The owner's PC). A trap for the Direct3D 11 front, seen by the ninth run's check: on real
  Direct3D on the RTX 3080, drawing on the immediate context after ExecuteCommandList(list, FALSE)
  without setting its state again (it is then cleared) hung the GPU, the device was removed
  (0x887A0005) and the driver reset. The resets of 2026-11-17 are traced in The owner's PC; one
  (16:44) has no known cause.
- Jobs that present need someone logged in at the desktop's console (the owner normally is): the
  hidden desktop lives in that session. Without it, `gpu-job --desktop` fails after 30 s with a
  message; `scenes`, `loop`, `wine-tests run` and `programs run` all use it.
- On the hidden desktop every window counts as occluded: Present returns DXGI_STATUS_OCCLUDED. Eight
  of Wine's tests fail on real Direct3D for that alone (see Measurements), and programs that stop
  drawing while occluded cannot be run there: Dear ImGui's Direct3D 11 example was tried and only
  test-presents after its first frame, so it was taken out of the list.
- Nineteen of Wine's tests (wine-excluded.txt) are never run here because they would change the
  owner's display (modes, fullscreen, gamma, cursor clipping); they need a machine whose display
  nobody uses. dxgi test_swapchain_backbuffer_index runs on WARP only (quarantined on the GPU).
- The census counts calls made through the program's objects, including those made by Windows' own
  libraries on them (most of the device's AddRef, Release, QueryInterface and GetPrivateData), and
  only on the device, contexts, swap chains and factories: methods of resources, views, states and
  queries are not counted. Factory calls made before the program's first device are not counted.
  A program that gets a context other than through D3D11CreateDevice(AndSwapChain),
  GetImmediateContext* or CreateDeferredContext* would not be counted on it. On Direct3D 12 the
  same holds for the device, queues and command lists: DXGI's own command list (one Reset, Close
  and ExecuteCommandLists a present, listed in census.txt's "made by Windows' own libraries"
  line) and DXGI's or the runtime's calls of the queue's Wait and Signal and the device's
  CreateCommandList, CreateCommandAllocator, Evict and MakeResident are in the counts. A Direct3D 12 device made other than through D3D12CreateDevice (the Agility SDK's
  ID3D12DeviceFactory) would not be counted at all.
- Godot's 3D platformer on Direct3D 12 differs from run to run in under 80 pixels by 1 (see
  Measurements), within the frame tolerance; its Direct3D 11 frames, Doom's and the 2D game's do
  not.
- bgfx-24-nbody's frames are not compared: its particles drift differently on every run on the
  RTX 3080 itself (about 230 and 630 of 3600 tiles at frames 60 and 120, and at frame 10 one
  pixel by 9 steps in one check of four), so it is run for its census, its presents and the
  libraries it loads only.
- atg-simplemsaa_pc draws a few edge pixels of its multisampled city differently on real
  Direct3D from run to run (up to 14 pixels of a frame, by up to 12 steps), in tiles that change
  from job to job more than within one: thirty runs, each its own job, found 4, 9 and 4 such tiles
  in its three frames and were still finding new ones after the twentieth. Its frames pass by the
  stray allowance (2, 11 and 11 pixels anywhere, frames 10, 120 and 60); a replacement that
  differs in a few more edge pixels than that would fail, and real Direct3D might too, rarely (20
  of 20 separate jobs passed, using at most 2). Cause not known (the frame clock gives it the same
  times on every run).
- On Direct3D 12, bgfx's overlay line of GPU time differs from run to run (the frame clock answers
  Direct3D 11's timestamp queries only; Direct3D 12's are resolved into buffers the program maps),
  so for bgfx-01-cubes-d3d12 and bgfx-38-bloom-d3d12 the rectangle left out of the comparison
  covers it as well as the memory line (x 0 to 319, y 208 to 271).
- Some counts of a Direct3D 12 program's census differ from run to run (Measurements, R2 third
  run): bgfx's texture uploads (CheckFeatureSupport, CreateFence, CopyTextureRegion,
  ResourceBarrier, Wait; by a few, and by up to a third in one check) and the device's AddRef and
  Release in all but the samples. Everything else was the same in every run, and the two samples'
  census was identical in all runs.
- The bgfx examples' GPU memory readout (one line of their overlay) is not compared: it changed
  between runs of real Direct3D, and a replacement reports its own memory. Other readouts of the
  overlay (frame time, GPU time, latency) are fixed by the frame clock and are compared.
- Godot's 3D game on ANGLE takes about 30 s, most of it compiling shaders, as each run starts with
  empty caches.
- Microsoft's Direct3D 12 samples run are the two simplest of D3D12HelloWorld. The Xbox-ATG-Samples
  PC12 samples and DirectX-Graphics-Samples' bigger ones are not in yet. The DirectX Tool Kit's own
  sample repository (walbourn/directxtk-samples) is not used, as its samples play sound with no way
  to turn it off, which would come out of the owner's speakers.
- Absent is decided from the device's own answers, so a front that reports less than it could
  is not caught by the scenes (only shown, in the count of absent outputs and each line's reason);
  what a front must report is the plan's (section 9) to check. Requirements are declared per scene
  and part (the lowest level, sample counts, format abilities, options) and were checked on real
  Direct3D asked for 9_3 to 11_1 on the GPU and 10_0 on WARP (no FAIL), but the RTX 3080 has every
  optional ability at 11_x, so a missing option check (one that real hardware lacks only on other
  adapters) would show as a FAIL on a front, not absent. The Direct3D 12 scenes cannot be capped
  like that (a Direct3D 12 device is 11_0 and up, its options the adapter's); their declared needs
  are sample counts, render-target support per format and reduction filtering.
- The device scene needs 11_0 whole (its answers are about 11_0 devices). Parts needing more
  than their scene are their own outputs, so the rest runs lower: BC1 to BC5 (textures-bc, 10_0)
  apart from BC6H and BC7 (textures-bc67, 11_0), the four render targets written without blending
  (targets-plain_*, 10_0) apart from blending set per target (10_1), and the indirect draws
  (draws-indirect, 11_0) apart from the other draws (10_0).
- Wine's test_create_device is excluded (its child process goes fullscreen), so the device scene
  (D3D11CreateDevice and D3D11CreateDeviceAndSwapChain with out parameters left out, empty, unknown
  and repeated level lists, driver types with and without an adapter, an undefined flag, SDK
  version 0, creation flags, the context and adapter of the device made) covers that ground; its
  answers are the same on the GPU and WARP. Direct3D 12's device creation has no such scene.
- A Godot program whose Direct3D fails ends only at the 120 s limit (Godot's message box waits on
  the hidden desktop), so such a FAIL takes two minutes.

- On 2026-11-17 SSH refused every connection for about 4 hours (09:45 to 13:52 Pacific time),
  each reset right after the server accepted the key exchange, before authentication; port 22
  answered throughout and the machine did not reboot. On Windows' OpenSSH that is where the
  server starts a process for the session, so a failure to start processes is a guess at the
  cause, not checked.
- SSH to the desktop has refused new connections (reset right after key exchange) for about 9 to 10
  minutes at a time and then recovered by itself, with no reboot and nothing in the OpenSSH or System
  logs. It happened twice in the first run (once after about 30 ssh sessions in 10 minutes) and
  again during the second independent check, when the checker had opened only about 4 new
  connections and an already open shared connection kept working throughout. So it is not this rig's
  connection count alone; other users of the machine may be the cause. Cause unknown; sshd_config sets
  no limits of its own. The scripts share one connection and wait up to 30 minutes for it (see How
  to run); in the third run the first connection was refused once and came up 15 s later.
- On 2026-11-17 at about 09:20 Pacific time the desktop's clock went back about 8 hours (job
  times printed 17:14 and then 09:22; not done by anything of ours, cause not looked into, as the
  machine's settings are not ours to touch). Cargo decides what to rebuild by file times, so
  builds after that kept artifacts from before it (stamped hours in the future) and ran old code
  of changed files until `cargo clean -p wcontainers-graphics-test -p
  wcontainers-graphics-test-forward` through `remote`. If the clock jumps back again, do the same.
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
In R2 the forwarding crate's windows dependency gained the feature Win32_Graphics_Direct3D12 and
the rig crate's Win32_System_Environment (for gt-chrome); no crate was added.
Real programs (built in C:\wc\programs, run for testing, never in a crate; licences read at the
source on 2026-11-17): microsoft/Xbox-ATG-Samples at ff629a6e (MIT: one LICENSE at its root,
GitHub's detection MIT, covering the samples, Kits (DirectXTK, ATGTK) and Media), added in R1's
seventh run; microsoft/DirectX-SDK-Samples at 07e3eaa1 (MIT, LICENSE file; only
C++/Direct3D11/Tutorials fetched), bgfx at 3aab3201, bx at 365db1e2 and bimg at f091e14e
(BSD-2-Clause each, per GitHub's licence detection of their LICENSE files). bgfx's
docs/license.rst lists its examples' third-party assets separately, several without a licence
(Uffizi from Paul Debevec's gallery, whose page reserves copyright, and the Stanford bunny among
them); since R1's ninth run bgfx is fetched without examples/assets and examples/runtime/meshes,
textures, font, images and text (partial clone, so those files are not downloaded either; the
earlier full checkout was deleted), and the examples run load none of them. What they draw beyond
bgfx's own code and shaders is the overlay's fonts, compiled into the examples
(examples/common/imgui): Roboto and Roboto Mono (Apache-2.0, as their own name tables say), the
Kenney icon font (CC0, its name table), Font Awesome 4.7 (SIL OFL 1.1, fontawesome.com/v4/license),
Font Awesome 7 Brands (fonts SIL OFL 1.1, its LICENSE.txt) and Codicons (CC-BY-4.0,
microsoft/vscode-codicons LICENSE); read on 2026-11-17. In R1's second run
the forwarding crate gained png 0.18.1 (MIT OR Apache-2.0, already in the lock file), anyhow and
windows (MIT OR Apache-2.0).
In R1's fifth run the forwarding crate gained retour 0.3.1 (BSD-2-Clause, read on crates.io and
its repository on 2026-11-17), for the frame clock's detours, with libudis86-sys 0.2.1 (MIT; it
builds udis86, BSD-2-Clause), mmap-fixed-fixed 0.1.3 (MIT), region 3.0.2 (MIT), slice-pool2 0.4.3
(MIT), generic-array 0.14.7 (MIT), typenum (MIT OR Apache-2.0), bitflags 1.3.2, winapi 0.3.9,
windows-sys 0.52, libc, cfg-if and once_cell (all MIT OR Apache-2.0), all test tooling, never
shipped. No new program was added.
In R1's third run (read at the source on 2026-11-17): Godot 4.7.2's official build (MIT; it
contains ANGLE, BSD-3-Clause, and the other libraries its COPYRIGHT.txt lists) and
godot-demo-projects at b761b4cd (MIT, LICENSE.md; the 2D platformer's README credits its music,
"Pompy" by Hubert Lamontagne, without a licence: the programs run without sound and no audio is
compared or kept, so it plays no part here); Chocolate Doom 3.1.1's released build (GPL-2.0,
testing only; it carries SDL2, SDL2_mixer and SDL2_net, zlib); Freedoom 0.13.0 (BSD-3-Clause,
COPYING.adoc; its SHA-256 matches the project's signed checksum file); vkd3d 2.1 test sources
(LGPL-2.1, testing only, in C:\wc\wine\vkd3d, never in the repository or a crate). No crate was
added.
In R2's second run (read at the source on 2026-11-17): microsoft/DirectX-Graphics-Samples at
68a2b1e9 (MIT, its LICENSE file and GitHub's detection; only Samples/Desktop/D3D12HelloWorld
fetched), and the two NuGet packages its projects import, downloaded at pinned hashes (the
SHA-512 nuget.org lists was checked on the Mac before the SHA-256 was pinned):
Microsoft.Direct3D.D3D12 1.618.3, the Agility SDK (its nuspec names LICENSE.txt, "Microsoft
Software License Terms, Microsoft DirectX", and requires no acceptance), and
Microsoft.Direct3D.DXC 1.8.2505.32 (its nuspec names LICENSE-LLVM.txt; the package also carries
LICENSE-MS.txt and LICENCE-MIT.txt). Both only build and run the samples on the desktop, never in
the repository or a crate. No crate was added; the forwarding crate's windows features are
unchanged.
pueue MIT OR Apache-2.0 (its Cargo.toml at v4.0.4), Git GPL-2.0, Node MIT, Rust MIT OR Apache-2.0, Visual Studio Build Tools and Windows SDK
under Microsoft's licence terms, Chrome for Testing under Google's terms.

## Next

R2 is done (third run: the findings of its first check fixed) and waits for its independent
check. Open points, for later:
- Direct3D 12 timestamps are not answered from the frame clock (bgfx's GPU time line is left out
  instead); only the two samples have a census checked against their source.
- The 19 Wine tests that need a display nobody uses (Known issues).
- More Direct3D 12 programs: the bigger DirectX-Graphics-Samples (they load assets and some need
  newer features) and Xbox-ATG-Samples' PC12 samples.
- A Direct3D 12 device made through the Agility SDK's ID3D12DeviceFactory is not counted (no
  program run does that yet).
