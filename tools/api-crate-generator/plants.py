#!/usr/bin/env python3
"""Planted defects in the generated glue, and the run that shows each one fails the tests.

    plants.py list <direct crate folder> <api name>      writes the plant list of one API's direct crate (stdout)
    plants.py run <plant list> [--only N,M] -- <test command ...>
                                                           applies each plant in turn to the tree (from the repository's
                                                           root), runs the test command, restores the file; a plant is
                                                           CAUGHT when the command fails, SURVIVED when it passes, and
                                                           INVALID when it does not compile

One plant per field of every copy function (data.rs: a field a decoder reads is read from zero bytes instead; a field an
encoder writes is not written), per argument of every call into the system (call.rs: a value passed as its type's
default, a pointer as null) and per argument of every call into the implementation (serve.rs: passed as its type's
default), and the named plants the independent checks asked for. The list is text, one plant a line, tab-separated:
id, file, the function the plant is in, the text replaced (with \\n for new lines), the text put in its place.
"""

import os
import re
import subprocess
import sys
import time


def functions(text):
    """Every function of a file, at any indentation (an `impl`'s methods too): name, start offset, end offset (just past
    its closing brace)."""
    found = []
    for match in re.finditer(r"^[ \t]*(?:pub(?:\([a-z]+\))? )?(?:unsafe )?(?:extern \"system\" )?fn ([A-Za-z0-9_]+)", text, re.M):
        depth, at = 0, match.end()
        while at < len(text):
            ch = text[at]
            if ch == "(":
                depth += 1
            elif ch == ")":
                depth -= 1
            elif ch == ";" and depth == 0:
                at = -1
                break
            elif ch == "{" and depth == 0:
                break
            at += 1
        if at < 0 or at >= len(text):
            continue
        end = matching(text, at)
        if end > 0:
            found.append((match.group(1), match.start(), end + 1))
    return found


def matching(text, open_at):
    """The offset of the bracket closing the one at `open_at`."""
    depth = 0
    pairs = {"(": ")", "[": "]", "{": "}"}
    closing = set(pairs.values())
    at = open_at
    while at < len(text):
        ch = text[at]
        if ch in pairs:
            depth += 1
        elif ch in closing:
            depth -= 1
            if depth == 0:
                return at
        at += 1
    return -1


def split_arguments(text):
    """Top-level comma-separated parts of an argument list, as (start, end) offsets into `text`."""
    parts, depth, start = [], 0, 0
    for at, ch in enumerate(text):
        if ch in "([{<":
            depth += 1
        elif ch in ")]}>":
            depth -= 1
        elif ch == "," and depth == 0:
            parts.append((start, at))
            start = at + 1
    if text[start:].strip():
        parts.append((start, len(text)))
    return [(s, e) for s, e in parts if text[s:e].strip()]


def codec_plants(path, text):
    plants = []
    for name, start, end in functions(text):
        body = text[start:end]
        if name.startswith("decode_") and "<" not in body.split("(", 1)[0]:
            size = re.search(r"/// Reads `[^`]+` from the (\d+) bytes", text[max(0, start - 300):start])
            if not size:
                continue
            literal = body.find("{", body.find("->"))
            inner_open = body.find("{", literal + 1)
            if inner_open < 0:
                continue
            inner_close = matching(body, inner_open)
            fields_text = body[inner_open + 1:inner_close]
            for field_start, field_end in split_arguments(fields_text):
                field = fields_text[field_start:field_end]
                field_name = field.strip().split(":", 1)[0].strip()
                if not re.fullmatch(r"[a-z_0-9]+", field_name):
                    continue
                replacement = f"\n            {field_name}: {name}(&[u8::MIN; {size.group(1)}]).{field_name}"
                plants.append((f"{name}.{field_name}", name, field, replacement))
        elif name.startswith("encode_"):
            open_brace = body.find("{", body.find(")"))
            close = matching(body, open_brace)
            statements = body[open_brace + 1:close]
            depth, begin = 0, 0
            for at, ch in enumerate(statements):
                if ch in "([{":
                    depth += 1
                elif ch in ")]}":
                    depth -= 1
                elif ch == ";" and depth == 0:
                    statement = statements[begin:at + 1]
                    field = re.search(r"value\.([a-z_0-9]+)", statement)
                    if field and statement.strip():
                        plants.append((f"{name}.{field.group(1)}", name, statement, ""))
                    begin = at + 1
    return [(path,) + plant for plant in plants]


def call_plants(path, text):
    plants = []
    for name, start, end in functions(text):
        body = text[start:end]
        for match in re.finditer(r"\.entry_points\(\)\s*\.([a-z_0-9]+)\)\s*\(", body):
            open_at = match.end() - 1
            close = matching(body, open_at)
            arguments = body[open_at + 1:close]
            for index, (s, e) in enumerate(split_arguments(arguments)):
                argument = arguments[s:e].strip()
                if argument.startswith("self.object.raw()"):
                    continue
                pointer = any(word in argument for word in ("as_ptr", "as_mut_ptr", "null", "cast::<", "raw()", "into_raw", "address"))
                new = "core::ptr::null_mut()" if pointer else "Default::default()"
                whole = body[match.start():close + 1]
                planted = whole[:open_at - match.start() + 1] + arguments[:s] + " " + new + arguments[e:] + ")"
                plants.append((path, f"{name}.{match.group(1)}.argument{index}", name, whole, planted))
    return plants


def serve_plants(path, text):
    plants = []
    for name, start, end in functions(text):
        body = text[start:end]
        for match in re.finditer(r"\.value\(\)\s*\.([a-z_0-9]+)\(", body):
            open_at = match.end() - 1
            close = matching(body, open_at)
            arguments = body[open_at + 1:close]
            for index, (s, e) in enumerate(split_arguments(arguments)):
                whole = body[match.start():close + 1]
                planted = whole[:open_at - match.start() + 1] + arguments[:s] + " Default::default()" + arguments[e:] + ")"
                plants.append((path, f"{name}.{match.group(1)}.argument{index}", name, whole, planted))
    return plants


# The plants the independent checks of 2026-11-27 asked for by name (batch-31-results.txt, check:graphics-api-direct3d).
NAMED = {
    "direct3d-12": [
        ("serve.rs", "checker.serve-getdesc-garbage", "resource_get_desc",
         "            Some((ret_val_bytes,))",
         "            Some(([0xAB_u8; size_of::<WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout>()],))"),
        ("call.rs", "checker.call-getdesc-zero", "get_desc",
         "        let ret_val = decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(&ret_val.0);",
         "        let ret_val = decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(\n            &[u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout>()],\n        );"),
        ("call.rs", "checker.call-heapdesc-skip", "get_desc",
         "        let ret_val =\n            decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(&ret_val.0);",
         "        let ret_val = decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(\n            &[u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12D3d12DescriptorHeapDescLayout>()],\n        );"),
    ],
}


def escape(text):
    return text.replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n")


def unescape(text):
    out, at = [], 0
    while at < len(text):
        if text[at] == "\\" and at + 1 < len(text):
            out.append({"n": "\n", "t": "\t", "\\": "\\"}[text[at + 1]])
            at += 2
        else:
            out.append(text[at])
            at += 1
    return "".join(out)


def list_plants(folder, api):
    rows = []
    for file, maker in (("data.rs", codec_plants), ("call.rs", call_plants), ("serve.rs", serve_plants)):
        path = os.path.join(folder, file)
        if not os.path.exists(path):
            continue
        with open(path) as handle:
            rows.extend(maker(path, handle.read()))
    for file, ident, function, old, new in NAMED.get(api, []):
        rows.append((os.path.join(folder, file), ident, function, old, new))
    for path, ident, function, old, new in rows:
        print("\t".join([ident, path, function, escape(old), escape(new)]))


def apply(text, function, old, new):
    """The text with `old` replaced by `new` once inside `function` (the first such function holding it)."""
    for name, start, end in functions(text):
        if name != function:
            continue
        at = text.find(old, start, end)
        if at >= 0:
            return text[:at] + new + text[at + len(old):]
    return None


def run(list_path, only, command):
    with open(list_path) as handle:
        rows = [line.rstrip("\n").split("\t") for line in handle if line.strip()]
    counts = {"CAUGHT": 0, "SURVIVED": 0, "INVALID": 0, "NOT FOUND": 0}
    for number, (ident, path, function, old, new) in enumerate(rows, 1):
        if only and number not in only:
            continue
        with open(path) as handle:
            original = handle.read()
        planted = apply(original, function, unescape(old), unescape(new))
        if planted is None:
            counts["NOT FOUND"] += 1
            print(f"PLANT {number} {ident}: NOT FOUND", flush=True)
            continue
        started = time.time()
        try:
            with open(path, "w") as handle:
                handle.write(planted)
            result = subprocess.run(command, capture_output=True, text=True)
        finally:
            with open(path, "w") as handle:
                handle.write(original)
        output = result.stdout + result.stderr
        if result.returncode == 0:
            verdict = "SURVIVED"
        elif re.search(r"^error(\[E\d+\])?: ", output, re.M) and "could not compile" in output:
            verdict = "INVALID"
        else:
            verdict = "CAUGHT"
        counts[verdict] += 1
        deciding = next((line for line in output.splitlines() if "panicked at" in line or "ERROR: AddressSanitizer" in line or line.startswith("error")), "")
        print(f"PLANT {number} {ident}: {verdict} status {result.returncode} {time.time() - started:.0f} s {deciding[:200]}", flush=True)
    print("PLANTS: " + ", ".join(f"{key} {value}" for key, value in counts.items()), flush=True)
    return 0


def main(arguments):
    if len(arguments) >= 3 and arguments[0] == "list":
        list_plants(arguments[1], arguments[2])
        return 0
    if len(arguments) >= 3 and arguments[0] == "run" and "--" in arguments:
        split = arguments.index("--")
        only = set()
        if "--only" in arguments[:split]:
            only = {int(n) for n in arguments[arguments.index("--only") + 1].split(",") if n.strip()}
        return run(arguments[1], only, arguments[split + 1:])
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
