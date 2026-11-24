"""Compares one program's replayed frames: the driver's with each of two real implementations', beside
how far those two differ from each other.

Usage: compare-frames.py <folder> <real> <other real> <driver> <tolerance> <kept frame>...,
with <folder>\\<implementation>\\<n>.png.

A pixel differs when any channel differs by more than the tolerance. A frame passes when the
driver's differs from one of the real implementations' in no more pixels than the two real ones
differ from each other. Writes comparison.txt in the folder and keeps the kept frames and each
implementation's frame where the driver is furthest from both; the rest are removed.
"""

import os
import sys

from PIL import Image, ImageChops

# The largest value of one channel of a frame's pixel.
CHANNEL_MAX = len(Image.new("L", (1, 1)).histogram()) - 1


def differing(a, b, tolerance):
    """Pixels of two frames differing beyond the tolerance, and the largest channel difference;
    frames of different sizes differ everywhere."""
    if a.size != b.size:
        width, height = a.size
        return width * height, CHANNEL_MAX
    red, green, blue = ImageChops.difference(a, b).split()
    largest = ImageChops.lighter(ImageChops.lighter(red, green), blue)
    histogram = largest.histogram()
    beyond = sum(histogram[tolerance + 1 :])
    most = max((value for value, count in enumerate(histogram) if count), default=0)
    return beyond, most


def frames(folder):
    return {
        int(os.path.splitext(name)[0]): os.path.join(folder, name)
        for name in os.listdir(folder)
        if name.endswith(".png")
    }


def main():
    folder, real, other, driver, tolerance, *kept = sys.argv[1:]
    tolerance = int(tolerance)
    shots = {name: frames(os.path.join(folder, name)) for name in (real, other, driver)}
    common = sorted(set(shots[real]) & set(shots[other]) & set(shots[driver]))
    lines = [
        f"frames: {real} {len(shots[real])}, {other} {len(shots[other])}, {driver} {len(shots[driver])}, compared {len(common)}"
    ]
    failed = []
    worst = None
    for n in common:
        image = {name: Image.open(shots[name][n]).convert("RGB") for name in shots}
        reals, reals_most = differing(image[real], image[other], tolerance)
        to_real, to_real_most = differing(image[driver], image[real], tolerance)
        to_other, to_other_most = differing(image[driver], image[other], tolerance)
        nearest = min(to_real, to_other)
        verdict = "pass" if nearest <= reals else "FAIL"
        if verdict == "FAIL":
            failed.append(n)
        if worst is None or nearest > worst[1]:
            worst = (n, nearest)
        lines.append(
            f"frame {n}: {verdict}; pixels beyond {tolerance}: {driver}-{real} {to_real} (largest {to_real_most}), "
            f"{driver}-{other} {to_other} (largest {to_other_most}), {other}-{real} {reals} (largest {reals_most})"
        )
    summary = (
        f"summary: {len(common) - len(failed)} of {len(common)} frames pass"
        + (f"; failing frames {failed}" if failed else "")
        + (f"; the driver furthest at frame {worst[0]} ({worst[1]} pixels)" if worst else "")
    )
    missing = len(shots[real]) - len(common)
    if missing:
        summary += f"; {missing} of {real}'s frames MISSING from the others"
    if not common:
        summary = "summary: no frame to compare"
    lines.insert(0, summary)
    with open(os.path.join(folder, "comparison.txt"), "w", encoding="utf8") as out:
        out.write("\n".join(lines) + "\n")
    print(f"compare-frames: {os.path.basename(folder)}: {summary}")
    keep = {int(n) for n in kept} | ({worst[0]} if worst else set())
    for name in shots:
        for n, path in shots[name].items():
            if n not in keep:
                os.remove(path)


if __name__ == "__main__":
    main()
