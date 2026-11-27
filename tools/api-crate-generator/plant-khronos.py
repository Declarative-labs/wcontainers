#!/usr/bin/env python3
"""Removes each check of the OpenGL ES and EGL glue in turn and runs the suite on the direct crates: every check must
make a test fail when it is gone. A plant is one check removed at one place (a call site of a generated method, or a
part of a shared helper); a check no plant can remove at a size a test can reach is listed with its reason.

    plant-khronos.py <work folder> [workers] [only]   run from the repository's root, inside its `nix develop`, with
                                                      EGL_PLATFORM, WCONTAINERS_EGL_FOLDER and the vendor file set

Prints one line per plant: CAUGHT (the tests that failed and the first message), MISSED, BROKEN (the plant did not
build: a defect of this script), then the LISTED checks and the totals."""

import os
import re
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

GL = "crates/opengl-es/implementation/direct"
EGL = "crates/egl/implementation/direct"

# Helpers a per-site plant calls in place of the check it removes; added to the file only in the planted copy.
GL_HELPERS = """
impl OpenGlEsContext {
    fn unpack_pointer_planted(&self, pixels: PixelUnpack<'_>, _: i32, _: i32, _: Option<i32>, _: u32, _: u32) -> Option<*const c_void> {
        Some(match pixels { PixelUnpack::Bytes(bytes) => bytes.as_ptr().cast(), PixelUnpack::Offset(offset) => core::ptr::without_provenance(usize::try_from(offset).unwrap_or(0)) })
    }
    fn pack_pointer_planted(&self, pixels: PixelPack<'_>, _: i32, _: i32, _: u32, _: u32) -> Option<*mut c_void> {
        Some(match pixels { PixelPack::Bytes(bytes) => bytes.as_mut_ptr().cast(), PixelPack::Offset(offset) => core::ptr::without_provenance_mut(usize::try_from(offset).unwrap_or(0)) })
    }
    fn compressed_pointer_planted(&self, pixels: CompressedPixelUnpack<'_>) -> Option<(*const c_void, i32)> {
        Some(match pixels {
            CompressedPixelUnpack::Bytes(bytes) => (bytes.as_ptr().cast(), i32::try_from(bytes.len()).unwrap_or(0)),
            CompressedPixelUnpack::Offset { offset, size } => (core::ptr::without_provenance(usize::try_from(offset).unwrap_or(0)), size),
        })
    }
    fn fence_address_any_context(&self, fence: &Fence) -> Option<*mut c_void> {
        self.fences.borrow().get(&fence.number).map(|&address| core::ptr::with_exposed_provenance_mut(address))
    }
    fn fence_address_even_deleted(&self, fence: &Fence) -> Option<*mut c_void> {
        if fence.context != self.number {
            return None;
        }
        EVER.lock().ok()?.get(&(self.number, fence.number)).map(|&address| core::ptr::with_exposed_provenance_mut(address))
    }
}
static EVER: std::sync::Mutex<BTreeMap<(u64, u64), usize>> = std::sync::Mutex::new(BTreeMap::new());
"""
GL_KEEP = ("        self.fences.borrow_mut().insert(number, address.expose_provenance());\n",
           "        self.fences.borrow_mut().insert(number, address.expose_provenance());\n        if let Ok(mut ever) = EVER.lock() { ever.insert((self.number, number), address.expose_provenance()); }\n")


def method_of(lines):
    """The method each line of a calls.rs is in."""
    current, out = None, []
    for line in lines:
        found = re.match(r"    fn ([a-z_0-9]+)\(", line)
        if found:
            current = found.group(1)
        out.append(current)
    return out


def gl_site_plants(text):
    """Per-site plants of the OpenGL ES calls.rs: (id, [(old line index, new line)], helpers wanted)."""
    lines = text.split("\n")
    methods = method_of(lines)
    plants, listed = [], []
    for at, (line, method) in enumerate(zip(lines, methods)):
        body = line.strip()
        def plant(kind, new, helpers=False):
            plants.append((f"{kind}:{method}", [(at, new)], helpers))
        if body.startswith("if !self.draw_ready()"):
            plant("draw-with-an-attribute-reading-no-buffer", "")
        found = re.match(r"let Some\((\w+)\) = runtime::counted::<(\w+)>\((\w+)\.len\(\), (\d+)\) else (.*)$", body)
        if found:
            name, ty, slice_, by, rest = found.groups()
            indent = line[: len(line) - len(line.lstrip())]
            if int(by) > 1:
                plant("count-must-divide", f"{indent}let {name} = runtime::counted::<{ty}>({slice_}.len() - {slice_}.len() % {by}, {by}).unwrap_or_default();")
            plant("count-is-the-slice", f"{indent}let Some({name}) = runtime::counted::<{ty}>({slice_}.len() + {by}, {by}) else {rest}")
            listed.append((f"count-fits:{method}", f"{slice_}'s count must fit {ty}: a slice of 2^31 elements or more, which no test allocates"))
        if "self.fence_address(sync)" in body:
            plant("fence-of-another-context", line.replace("self.fence_address(sync)", "self.fence_address_any_context(sync)"), True)
            plant("fence-deleted", line.replace("self.fence_address(sync)", "self.fence_address_even_deleted(sync)"), True)
        if body == "self.forget_fence(sync);":
            plant("fence-forgotten-on-delete", "")
        found = re.search(r"unsafe \{ entry\(.*\b(\w+)_room\.as_(mut_)?ptr\(\)", body)
        if found:
            room = found.group(1)
            kind = "room-out" if found.group(2) else "room-in"
            plant(kind, line.replace(f"{room}_room.as_{found.group(2) or ''}ptr()", f"{room}.as_{found.group(2) or ''}ptr().cast()"))
        if body.startswith("if [constants::COMPRESSED_TEXTURE_FORMATS"):
            plant("variable-list-refused", "")
        for helper in ("unpack_pointer", "pack_pointer", "compressed_pointer"):
            if f"self.{helper}(" in body:
                plant("pixels-checked", line.replace(f"self.{helper}(", f"self.{helper}_planted("), True)
        found = re.search(r"self\.offset_into\(constants::\w+, (\w+)\)", body)
        if found:
            plant("offset-needs-a-buffer", line.replace(found.group(0), f"Some(core::ptr::without_provenance(usize::try_from({found.group(1)}).unwrap_or(0)))"))
        if body.startswith("if data.is_some_and("):
            plant("bytes-at-least-the-size", "")
        if body.startswith("if !self.has_extension("):
            plant("extension-named", "")
        found = re.match(r"let Ok\((\w+)\) = isize::try_from\(\w+\)", body)
        if found:
            listed.append((f"isize:{method}:{found.group(1)}", "an i64 that does not fit isize: none on a 64-bit target, the only kind the suite runs on"))
        if "i32::try_from(text.len())" in body:
            listed.append((f"text-length:{method}", "a text of 2^31 bytes or more, which no test allocates"))
        if re.match(r"let Some\(entry\) = entries\.\w+ else", body):
            listed.append((f"entry-missing:{method}", "the library gives no entry point: Mesa gives every one the set carries"))
    return plants, listed


SHARED_GL = [
    ("pixels-long-enough", "Some(needed) if needed <= held => true,", "Some(_) => true,"),
    ("pixels-row-length", "let row_pixels = if store.row_length > 0 { count(store.row_length)? } else { width };", "let row_pixels = width;"),
    ("pixels-alignment", "let alignment = count(store.alignment)?.max(1);", "let alignment = 1;"),
    ("pixels-skip-rows", ".checked_add(count(store.skip_rows)?)?", ""),
    ("pixels-skip-pixels", "let last_row_end = count(store.skip_pixels)?.checked_add(width)?.checked_mul(pixel)?;", "let last_row_end = width.checked_mul(pixel)?;"),
    ("pixels-image-height", "Some(_) => (if store.image_height > 0 { count(store.image_height)? } else { height }, count(store.skip_images)?),",
     "Some(_) => (height, count(store.skip_images)?),"),
    ("pixels-skip-images", "Some(_) => (if store.image_height > 0 { count(store.image_height)? } else { height }, count(store.skip_images)?),",
     "Some(_) => (if store.image_height > 0 { count(store.image_height)? } else { height }, 0_usize),"),
    ("pixels-images", "Some(depth) => count(depth)?,", "Some(_) => 1,"),
    ("pixels-format-known", "let Some(pixel) = pixel_bytes(format, type_) else {\n            self.refuse(constants::INVALID_ENUM);\n            return false;\n        };",
     "let pixel = pixel_bytes(format, type_).unwrap_or(1);"),
    ("pixels-unpack-bytes-need-no-buffer", "PixelUnpack::Bytes(bytes) => {\n                if bound {", "PixelUnpack::Bytes(bytes) => {\n                if false {"),
    ("pixels-pack-bytes-need-no-buffer", "PixelPack::Bytes(bytes) => {\n                if bound {", "PixelPack::Bytes(bytes) => {\n                if false {"),
    ("pixels-compressed-bytes-need-no-buffer", "(false, Some(size)) =>", "(_, Some(size)) =>"),
    ("offset-needs-a-buffer-shared", "(true, Ok(offset)) =>", "(_, Ok(offset)) =>"),
    ("draw-every-attribute", ".min(MOST_ATTRIBUTES)", ".min(1)"),
    ("draw-attribute-buffer", "room[0] != 0\n        })", "true\n        })"),
    ("fence-context-shared", "if fence.context != self.number {", "if false {"),
    ("context-lost", "if self.binding.enter() {", "if { let _ = self.binding.enter(); true } {"),
    ("count-must-divide-shared", "if by == 0 || !length.is_multiple_of(by) {", "if by == 0 {"),
    ("extensions-bounded", ".unwrap_or(0).min(MOST_EXTENSIONS);", ".unwrap_or(0);"),
]

SHARED_EGL = [
    ("egl-display-of-this-library", "(display.library == self.identity()).then(", "true.then("),
    ("egl-config-of-this-library", "(config.library == self.identity() && config.display", "(config.display"),
    ("egl-config-of-this-display", "config.library == self.identity() && config.display == display.expose_provenance()", "config.library == self.identity()"),
    ("egl-held-of-this-library", "(held.library == self.identity() && held.display", "(held.display"),
    ("egl-held-of-this-display", "held.library == self.identity() && held.display == display.expose_provenance() && held.alive", "held.library == self.identity() && held.alive"),
    ("egl-held-alive", "held.display == display.expose_provenance() && held.alive.load(Ordering::Acquire))", "held.display == display.expose_provenance())"),
    ("egl-key-has-a-value", "[_] => return None,", "[_] => break,"),
    ("egl-list-ends-in-none", "        out.push(constants::NONE);\n", ""),
    ("egl-forced-pairs", "        pairs.extend_from_slice(forced);\n", ""),
    ("egl-binding-context-alive-while-current", "        if !self.context.alive.load(Ordering::Acquire) {\n            return false;\n        }\n        let entries", "        let entries"),
]

# Plants of a plain value the glue must pass through as given (NEEDED "After GI7" item 1): the face, and an argument
# exchanged or changed. Each is an (id, old, new) of the OpenGL ES calls.rs.
PASSED_GL = [
    ("face-passed:stencil_func_separate", "unsafe { entry(face, func, ref_, mask) }", "unsafe { entry(constants::FRONT_AND_BACK, func, ref_, mask) }"),
    ("mask-passed:stencil_func_separate", "unsafe { entry(face, func, ref_, mask) }", "unsafe { entry(face, func, ref_, mask >> 1) }"),
    ("face-passed:stencil_op_separate", "unsafe { entry(face, sfail, dpfail, dppass) }", "unsafe { entry(constants::FRONT_AND_BACK, sfail, dpfail, dppass) }"),
    ("order-passed:stencil_op_separate", "unsafe { entry(face, sfail, dpfail, dppass) }", "unsafe { entry(face, dppass, dpfail, sfail) }"),
]

# Plants of a value written as some other number than the registry declares (NEEDED "After GI7" item 2): each is
# the value's line of the OpenGL ES constants.rs and a number no OpenGL ES value is.
VALUES_GL = [
    ("CLIP_DISTANCE0_EXT", "0x3000"), ("TEXTURE_MAX_ANISOTROPY_EXT", "0x84fe"), ("MAX_TEXTURE_MAX_ANISOTROPY_EXT", "0x84ff"),
    ("MIRROR_CLAMP_TO_EDGE_EXT", "0x8743"), ("SRC1_COLOR_EXT", "0x88f9"), ("SRC1_ALPHA_EXT", "0x8589"),
    ("ONE_MINUS_SRC1_COLOR_EXT", "0x88fa"), ("ONE_MINUS_SRC1_ALPHA_EXT", "0x88fb"),
]
UNDECLARED = "0xfff0"

# Plants of the direct crates' additions (NEEDED "After GI7" items 5 and 7): each an (id, file, old, new).
ADDED = [
    ("owned-context-destroyed", GL + "/implementation.rs", "        let _ = owned.egl.destroy_context(&owned.display, &owned.context);\n", ""),
    ("owned-draw-surface-destroyed", GL + "/implementation.rs", "            let _ = owned.egl.destroy_surface(&owned.display, draw);\n", ""),
    ("library-kept", EGL + "/implementation.rs", "        keep(folder, &loaded);\n", ""),
]

LISTED_SHARED = [
    ("extensions-bounded", "a library answering more extensions than 4096: Mesa answers its few hundred; the fuzzing's lying library answers up to 2^31 (a run without the bound does not end)"),
    ("egl-gate", "a destroy runs while no other call of the library is in flight (the gate): a race, which no single-threaded test makes"),
    ("egl-config-of-this-display", "EGL gives one default display per library (eglGetDisplay of EGL_DEFAULT_DISPLAY only): no test holds two displays of one library"),
    ("egl-held-of-this-display", "as egl-config-of-this-display"),
    ("egl-client-get-all", "a library without EGL_KHR_client_get_all_proc_addresses: Mesa names it"),
    ("egl-handles-written", "a slot past the count EGL wrote: also left None by the null check after it (two checks of one fact; only both removed shows)"),
    ("egl-handles-null", "a null handle in the count EGL wrote: EGL writes none"),
    ("value:UNMASKED_RENDERER_WEBGL", "WebGL's own value (WEBGL_debug_renderer_info): no native OpenGL ES context answers it, so no test over a native library can tell a wrong number; the browser implementation's test reads it"),
    ("owned-read-surface-destroyed-once", "a read surface that is also the draw surface is destroyed once: a second destroy of its ended handle is refused by the glue, so nothing a test can see changes"),
    ("egl-binding-alive-before-make-current", "a context or surface destroyed and not current: the check under the gate refuses it, and without it eglMakeCurrent refuses the destroyed handle (Mesa checks its handles), on which the glue refuses the call alike"),
]


def egl_site_plants(text):
    lines = text.split("\n")
    methods = method_of(lines)
    plants = []
    for at, (line, method) in enumerate(zip(lines, methods)):
        body = line.strip()
        def plant(kind, new):
            plants.append((f"{kind}:{method}", [(at, new)], False))
        if "self.display_address(handle)" in body:
            plant("egl-display", line.replace("self.display_address(handle)", "Some(core::ptr::with_exposed_provenance_mut::<c_void>(handle.address))"))
        if "self.config_address(handle, dpy)" in body:
            plant("egl-config", line.replace("self.config_address(handle, dpy)", "Some(core::ptr::with_exposed_provenance_mut::<c_void>(handle.address))"))
        found = re.search(r"let (\w+) = match \w+ \{ None => core::ptr::null_mut\(\), Some\(handle\) => match self\.held_address\(&handle\.held, dpy\)", body)
        if found or body.startswith("let Some(") and "self.held_address(&handle.held, dpy)" in body:
            name = found.group(1) if found else re.match(r"let Some\((\w+)\)", body).group(1)
            plants.append((f"egl-handle:{method}:{name}", [(at, line.replace("self.held_address(&handle.held, dpy)", "Some(core::ptr::with_exposed_provenance_mut::<c_void>(handle.held.address))"))], False))
        if "runtime::attribute_list(" in body:
            plant("egl-attribute-list", "")
        if body == "if result != 0 { handle.held.end(); }":
            plant("egl-destroyed", "")
    return plants


def apply(root, plant):
    """Writes the planted files into the worker's tree; returns False when an old text is not found exactly once."""
    _, edits = plant
    for path, changes in edits.items():
        full = os.path.join(root, path)
        with open(full) as f:
            text = f.read()
        for old, new in changes:
            if isinstance(old, int):
                lines = text.split("\n")
                lines[old] = new
                text = "\n".join(lines)
            elif text.count(old) != 1:
                return False
            else:
                text = text.replace(old, new)
        with open(full, "w") as f:
            f.write(text)
    return True


def plants_of(root):
    with open(os.path.join(root, GL, "calls.rs")) as f:
        gl_calls = f.read()
    with open(os.path.join(root, EGL, "calls.rs")) as f:
        egl_calls = f.read()
    plants, listed = [], []
    site, site_listed = gl_site_plants(gl_calls)
    listed += site_listed
    for name, changes, helpers in site:
        edits = {f"{GL}/calls.rs": changes}
        if helpers:
            edits[f"{GL}/implementation.rs"] = [(GL_KEEP[0], GL_KEEP[1]), ("\nimpl OpenGlEsContext {\n    /// Makes the context current", GL_HELPERS + "\nimpl OpenGlEsContext {\n    /// Makes the context current")]
        plants.append((name, edits))
    for name, old, new in SHARED_GL:
        plants.append((name, {f"{GL}/implementation.rs": [(old, new)]}))
    for name, old, new in PASSED_GL:
        plants.append((name, {f"{GL}/calls.rs": [(old, new)]}))
    for name, path, old, new in ADDED:
        plants.append((name, {path: [(old, new)]}))
    for name, number in VALUES_GL:
        line = f"pub const {name}: u32 = {number};"
        plants.append((f"value:{name}", {"crates/opengl-es/constants.rs": [(line, line.replace(number, UNDECLARED))]}))
    for name, changes, _ in egl_site_plants(egl_calls):
        plants.append((name, {f"{EGL}/calls.rs": changes}))
    for name, old, new in SHARED_EGL:
        plants.append((name, {f"{EGL}/implementation.rs": [(old, new)]}))
    plants.append(("egl-forced-robust-access", {f"{EGL}/calls.rs": [("const FORCED_EGL_CREATE_CONTEXT: &[(i32, i32)] = &[(constants::CONTEXT_OPENGL_ROBUST_ACCESS, constants::TRUE)];",
                                                                       "const FORCED_EGL_CREATE_CONTEXT: &[(i32, i32)] = &[];")]}))
    plants.append(("egl-handles-written", {f"{EGL}/calls.rs": [("(result != 0 && at < made && !address.is_null())", "(result != 0 && !address.is_null())")]}))
    plants.append(("egl-handles-null", {f"{EGL}/calls.rs": [("(result != 0 && at < made && !address.is_null())", "(result != 0 && at < made)")]}))
    plants.append(("egl-handles-written-and-null", {f"{EGL}/calls.rs": [("(result != 0 && at < made && !address.is_null())", "(result != 0)")]}))
    listed += LISTED_SHARED
    return plants, listed


def run(root, target, plant, jobs):
    name, edits = plant
    # Each plant starts from the pristine files.
    for path in {p for p in edits}:
        shutil.copy(os.path.join(os.getcwd(), path), os.path.join(root, path))
    if not apply(root, (name, edits)):
        return name, "BROKEN", "an old text not found exactly once"
    environment = dict(os.environ, CARGO_TARGET_DIR=target)
    try:
        done = subprocess.run(["cargo", "test", "-q", "-j", str(jobs), "-p", "wcontainers-opengl-es-direct", "--", "--test-threads=4"],
                              cwd=root, env=environment, capture_output=True, text=True, timeout=600)
        output, status = done.stdout + done.stderr, done.returncode
    except subprocess.TimeoutExpired:
        output, status = "", "timeout"
    for path in {p for p in edits}:
        shutil.copy(os.path.join(os.getcwd(), path), os.path.join(root, path))
    if status == 0:
        return name, "MISSED", ""
    if "error[E" in output or "could not compile" in output:
        first = next((line for line in output.split("\n") if line.startswith("error")), "")
        return name, "BROKEN", first
    failed = sorted(set(re.findall(r"^    ([\w:]+)$", output, re.M)) | set(re.findall(r"test ([\w:]+) \.\.\. FAILED", output)))
    message = next((line.strip() for line in output.split("\n") if "panicked at" in line or "assertion" in line or "signal" in line), "")
    following = output.split(message, 1)[1].strip().split("\n")[0] if message and message in output else ""
    return name, "CAUGHT", f"status {status}; {', '.join(failed) or 'the test process ended'}; {message} {following}".strip()


def main():
    work = sys.argv[1]
    workers = int(sys.argv[2]) if len(sys.argv) > 2 else 6
    only = sys.argv[3] if len(sys.argv) > 3 else ""
    plants, listed = plants_of(os.getcwd())
    plants = [plant for plant in plants if only in plant[0]]
    roots = []
    for index in range(workers):
        root = os.path.join(work, f"tree{index}")
        if not os.path.isdir(root):
            subprocess.run(["rsync", "-a", "--exclude", "target", "--exclude", ".git", "--exclude", ".gack", "./", root + "/"], check=True)
        roots.append((root, os.path.join(work, f"target{index}")))
    jobs = max(1, 12 // workers)

    def worker(index):
        root, target = roots[index]
        return [run(root, target, plant, jobs) for plant in plants[index::workers]]

    with ThreadPoolExecutor(workers) as pool:
        results = [result for batch in pool.map(worker, range(workers)) for result in batch]
    order = {plant[0]: at for at, plant in enumerate(plants)}
    totals = {}
    for name, verdict, detail in sorted(results, key=lambda result: order[result[0]]):
        totals[verdict] = totals.get(verdict, 0) + 1
        print(f"{verdict} {name} {detail}")
    for name, reason in listed:
        print(f"LISTED {name}: {reason}")
    print("TOTALS " + " ".join(f"{verdict} {count}" for verdict, count in sorted(totals.items())) + f" LISTED {len(listed)}")


if __name__ == "__main__":
    main()
