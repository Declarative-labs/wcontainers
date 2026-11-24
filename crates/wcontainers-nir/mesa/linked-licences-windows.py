"""The licence of every file linked into the Windows driver, opengl32.dll and libgallium_wgl.dll, from
the build's own dependency lists:
- Mesa: every source ninja's graph makes the two libraries from (`ninja -t inputs`, whole archives,
  so a file the linker drops is listed too), with its SPDX tag or the licence text it carries; a
  file Mesa's build writes is "generated", and the generators' inputs are listed with theirs;
- the libraries Meson links from outside the tree (LLVM for llvmpipe), from its dependency list;
- wcontainers_opengl.lib: every crate Cargo links into it for the Windows target (normal
  dependencies from wcontainers-opengl), with its declared licence, and the licence files of the
  native sources vendored inside those crates (every licence file below a workspace crate's own
  folder, whether its build compiles that source or not).

    python linked-licences-windows.py <Mesa build folder> <repository> <head lines> [output file]

where <head lines> is how many lines at the top of a file are searched for its licence.

One line a file: licence, kind, path. Prints the count of each licence.
"""
import argparse
import collections
import json
import os
import re
import shutil
import subprocess
import sys

LIBRARIES = ("libgallium_wgl.dll", "opengl32.dll")
TARGET = "x86_64-pc-windows-msvc"
DRIVER = "wcontainers-opengl"
# Files whose code is built into the libraries; other inputs only feed the generators.
COMPILED = {".c", ".cc", ".cpp", ".cxx", ".h", ".hpp", ".S", ".asm", ".def", ".rc", ".inc"}
SPDX = re.compile(r"SPDX-License-Identifier:\s*(.+?)\s*(\*/|-->|#)?\s*$")
TEXTS = (
    ("Permission is hereby granted, free of charge", "MIT text"),
    ("Redistribution and use in source and binary forms", "BSD text"),
    ("Licensed under the Apache License", "Apache-2.0 text"),
    ("Apache License\n", "Apache-2.0 text"),
    ("see copyright notice in zlib.h", "Zlib (zlib.h)"),
    ("Boost Software License", "BSL-1.0 text"),
)
# Meson's ninja: the one installed beside this interpreter, else the first on the path.
NINJA = shutil.which("ninja", path=os.path.join(sys.prefix, "Scripts")) or shutil.which("ninja") or "ninja"
LICENCE_FILES = re.compile(r"^(LICEN[SC]E|COPYING)(\.[A-Za-z]+)?$")


# Lines at the top of a file searched for its licence (the script's argument).
head_lines = None


def licence_of(path):
    try:
        with open(path, errors="replace") as f:
            head = [line for _, line in zip(range(head_lines), f)]
    except OSError:
        return "unreadable"
    for line in head:
        m = SPDX.search(line)
        if m:
            return m.group(1)
    text = "".join(head)
    return next((name for needle, name in TEXTS if needle in text), "no licence text")


def under(path, root):
    return os.path.normcase(os.path.abspath(path)).startswith(os.path.normcase(root) + os.sep)


def mesa(build, lines):
    info = json.load(open(os.path.join(build, "meson-info", "meson-info.json")))
    source = os.path.abspath(info["directories"]["source"])
    targets = []
    for folder, _, files in os.walk(os.path.join(build, "src")):
        targets += [os.path.relpath(os.path.join(folder, f), build) for f in files if f in LIBRARIES]
    if sorted(os.path.basename(t) for t in targets) != sorted(LIBRARIES):
        sys.exit(f"linked-licences-windows: the libraries were not all found: {targets}")
    inputs = subprocess.run(
        [NINJA, "-C", build, "-t", "inputs", *targets], check=True, capture_output=True, encoding="utf8", errors="replace"
    ).stdout.split()
    for path in sorted(set(inputs)):
        full = os.path.normpath(os.path.join(build, path))
        compiled = os.path.splitext(full)[1] in COMPILED
        if under(full, source):
            kind = "mesa" if compiled else "mesa generator input"
            lines.append((licence_of(full), kind, os.path.relpath(full, source)))
        elif under(full, build) and compiled:
            lines.append(("generated", "mesa", os.path.relpath(full, build)))
    return targets


def outside(build, lines):
    for dependency in json.load(open(os.path.join(build, "meson-info", "intro-dependencies.json"))):
        if dependency.get("name", "").lower() != "llvm":
            continue
        headers = [re.sub(r"^(-isystem|-I|/I)", "", a) for a in dependency.get("compile_args", [])
                   if a.startswith(("-isystem", "-I", "/I"))]
        tag = next(
            (licence_of(os.path.join(h, "llvm", "IR", "Module.h")) for h in headers
             if os.path.exists(os.path.join(h, "llvm", "IR", "Module.h"))),
            "no header read",
        )
        for library in dependency.get("link_args", []):
            if library.lower().endswith(".lib"):
                lines.append((tag, "llvm " + dependency.get("version", ""), library))


def cargo(repository, lines):
    metadata = json.loads(subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--filter-platform", TARGET,
         "--manifest-path", os.path.join(repository, "Cargo.toml")],
        check=True, capture_output=True, encoding="utf8", errors="replace",
    ).stdout)
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    start = next(i for i, p in packages.items() if p["name"] == DRIVER)
    linked, todo = set(), [start]
    while todo:
        node = nodes[todo.pop()]
        if node["id"] in linked:
            continue
        linked.add(node["id"])
        for dep in node["deps"]:
            if any(k["kind"] is None for k in dep["dep_kinds"]):
                todo.append(dep["pkg"])
    for i in sorted(linked, key=lambda i: packages[i]["name"]):
        p = packages[i]
        lines.append((p.get("license") or "no licence declared", "crate", f"{p['name']} {p['version']}"))
        root = os.path.dirname(p["manifest_path"])
        if not under(root, os.path.abspath(repository)):
            continue
        # Native sources vendored inside one of the workspace's crates (a submodule's licence files).
        for folder, folders, files in os.walk(root):
            folders[:] = [f for f in folders if f not in ("target", ".git")]
            if folder == root:
                continue
            for f in files:
                if LICENCE_FILES.match(f):
                    path = os.path.join(folder, f)
                    lines.append((licence_of(path), f"vendored in {p['name']}", os.path.relpath(path, repository)))


def main():
    global head_lines
    parser = argparse.ArgumentParser(description="The licence of every file linked into the Windows driver.")
    parser.add_argument("build", help="Mesa's build folder")
    parser.add_argument("repository")
    parser.add_argument("head_lines", type=int, help="lines at the top of a file searched for its licence")
    parser.add_argument("output", nargs="?", help="the list's file (linked-licences-windows.txt in the build folder)")
    arguments = parser.parse_args()
    build, repository = os.path.abspath(arguments.build), os.path.abspath(arguments.repository)
    head_lines = arguments.head_lines
    out = arguments.output or os.path.join(build, "linked-licences-windows.txt")
    lines = []
    targets = mesa(build, lines)
    outside(build, lines)
    cargo(repository, lines)
    with open(out, "w") as f:
        f.write("".join(f"{l}\t{k}\t{p}\n" for l, k, p in lines))
    counts = collections.Counter((k.split(" ")[0] if k.startswith(("llvm", "vendored")) else k, l) for l, k, _ in lines)
    for (kind, licence), n in sorted(counts.items()):
        print(f"linked-licences-windows: {kind}: {n} {licence}")
    print(f"linked-licences-windows: {len(lines)} lines for {', '.join(targets)}, {out}")


main()
