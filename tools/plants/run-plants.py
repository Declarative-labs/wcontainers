# Runs a committed plant list: each plant changes one place of one file (an exact text, which must occur once),
# alone, in a copy of the repository, and runs the list's test command there. A plant is CAUGHT when the command
# fails, SURVIVED when it passes, DID NOT BUILD when the change does not compile (a plant to fix, never counted as
# caught). Prints one line a plant and the totals; exits 0 only when every plant was caught.
#
#   python3 -I tools/plants/run-plants.py <plant list> [--workers N] [--only name,name] [--work folder]
#
# Run from the repository's root, inside its `nix develop`, with what the list's test command needs in the
# environment (for the Mesa crates, WCONTAINERS_MESA and WCONTAINERS_SPIRV_TOOLS). The list's format:
#
#   test: <shell command run in the copy's root>
#   plant: <name>          one per plant, then
#   file: <path from the repository's root>
#   old:                   the exact text, on the lines that follow
#   new:                   its replacement, on the lines that follow
#   end
#
# Lines starting with `#` outside a plant's old and new texts are comments.
import os, re, shutil, subprocess, sys, threading


def read(path):
    test, plants, plant, field = None, [], None, None
    for line in open(path).read().split("\n"):
        if field in ("old", "new") and line not in ("new:", "end"):
            plant[field].append(line)
            continue
        if line.startswith("#") or (not line.strip() and field is None):
            continue
        if line.startswith("test: "):
            test = line[len("test: "):]
        elif line.startswith("plant: "):
            plant = {"name": line[len("plant: "):], "old": [], "new": []}
            field = None
        elif line.startswith("file: "):
            plant["file"] = line[len("file: "):]
        elif line == "old:":
            field = "old"
        elif line == "new:":
            field = "new"
        elif line == "end":
            plant["old"], plant["new"] = "\n".join(plant["old"]), "\n".join(plant["new"])
            plants.append(plant)
            plant, field = None, None
        else:
            sys.exit(f"{path}: a line outside the format: {line!r}")
    if test is None:
        sys.exit(f"{path}: no test command")
    return test, plants


def main():
    args = sys.argv[1:]
    if not args:
        sys.exit(__doc__ or "usage: run-plants.py <plant list> [--workers N] [--only names] [--work folder]")
    listed = args[0]
    workers = int(args[args.index("--workers") + 1]) if "--workers" in args else 4
    only = set(args[args.index("--only") + 1].split(",")) if "--only" in args else None
    work = os.path.abspath(args[args.index("--work") + 1]) if "--work" in args else os.path.abspath("target/plants")
    tree = os.getcwd()
    test, plants = read(listed)
    plants = [p for p in plants if only is None or p["name"] in only]
    names = [p["name"] for p in plants]
    if len(set(names)) != len(names):
        sys.exit("two plants share a name")
    for plant in plants:
        text = open(os.path.join(tree, plant["file"])).read()
        if text.count(plant["old"]) != 1:
            sys.exit(f"PLANT {plant['name']}: its text is in {plant['file']} {text.count(plant['old'])} times, not once")
    os.makedirs(work, exist_ok=True)
    queue = list(range(len(plants)))
    lock = threading.Lock()
    results = {}

    def worker(k):
        copy = os.path.join(work, f"tree{k}")
        subprocess.run(["rsync", "-a", "--delete", "--exclude", "/target", "--exclude", "/.git", tree + "/", copy + "/"], check=True)
        env = dict(os.environ, CARGO_TARGET_DIR=os.path.join(work, f"target{k}"))
        while True:
            with lock:
                if not queue:
                    return
                number = queue.pop(0)
            plant = plants[number]
            path = os.path.join(copy, plant["file"])
            original = open(os.path.join(tree, plant["file"])).read()
            open(path, "w").write(original.replace(plant["old"], plant["new"]))
            try:
                run = subprocess.run(["bash", "-c", test], cwd=copy, env=env, capture_output=True, text=True, timeout=7200)
                text, status = run.stdout + run.stderr, run.returncode
            except subprocess.TimeoutExpired:
                text, status = "timed out", 124
            open(path, "w").write(original)
            # A compile error (rustc's `error[E..]` or `could not compile`, a C compiler's `error:` inside a build
            # script's output) is a plant that did not build; cargo's own `error: test failed` is a caught plant.
            if "could not compile" in text or re.search(r"^error\[E\d+\]", text, re.M) or "failed to run custom build command" in text:
                verdict = "DID NOT BUILD " + " | ".join(l for l in text.splitlines() if l.startswith("error"))[:300]
            elif status == 0:
                verdict = "SURVIVED"
            else:
                failed = sorted(set(re.findall(r"^test (\S+) \.\.\. FAILED", text, re.M)))
                verdict = "CAUGHT by " + (" ".join(failed) if failed else f"status {status}")
            with lock:
                results[number] = verdict
                print(f"PLANT {plant['name']}: {verdict}", flush=True)

    threads = [threading.Thread(target=worker, args=(k,)) for k in range(min(workers, max(len(plants), 1)))]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    caught = sum(1 for v in results.values() if v.startswith("CAUGHT"))
    survived = sum(1 for v in results.values() if v == "SURVIVED")
    unbuilt = sum(1 for v in results.values() if v.startswith("DID NOT"))
    print(f"PLANTS {len(results)}: caught {caught}, survived {survived}, did not build {unbuilt}", flush=True)
    sys.exit(0 if caught == len(plants) else 1)


main()
