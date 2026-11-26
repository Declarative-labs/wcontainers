"""Regenerates the A64 instruction list with Arm's machine-readable release as its primary source.

Usage: python3 -I from_arm_release.py <Instructions.json> <LLVM's list.tsv> <a64-instructions.tsv> <differences.tsv>

Arm's release (AARCHMRS_OPENSOURCE_A_profile_FAT-2026-09, BSD-3-Clause, `LICENSE-AARCHMRS.txt`) gives one row per A64
encoding: its fixed and should-be bits, its features, and the operand values its encoding conditions exclude. The
list as LLVM 23.1.5 gave it (Apache-2.0 WITH LLVM-exception, `LICENSE-LLVM.txt`), read from the same file, is the
cross-check and gives what Arm's release leaves out: the meaning of each operand field, the values llvm-mc rejects
(the decode pseudocode's UNDEFINED cases, absent from the release), an example word, the class and the EL0 behavior.
Every difference between the two sources goes to the differences file with the
reason the list follows the one it does.
"""

import collections
import json
import sys

WORD_BITS = 32
ALL_BITS = (1 << WORD_BITS) - 1

# Arm encodings the list names itself, where Arm's one encoding holds several LLVM records and a model refers to it.
NAMES = {
    "TBZ_only_testbranch": "Tbz",
    "TBNZ_only_testbranch": "Tbnz",
}
# The operand fields of those merged encodings, in the list's notation: TBZ's bit number is b5:b40 of a 64-bit read.
MERGED_FIELDS = {
    "TBZ_only_testbranch": "bit_off@31:31,23:19=read operand tbz_imm0_63; target@18:5=read operand am_tbrcond; "
    "Rt@4:0=read operand GPR64",
    "TBNZ_only_testbranch": "bit_off@31:31,23:19=read operand tbz_imm0_63; target@18:5=read operand am_tbrcond; "
    "Rt@4:0=read operand GPR64",
}
# The class of an encoding Arm alone has, by its top-level group, where no listed sibling shares its group.
CLASS_BY_GROUP = {
    "sve": "Sve",
    "sme": "Sme",
    "ldst": "LoadsAndStores",
    "dpimm": "DataProcessingImmediate",
    "dpreg": "DataProcessingRegister",
    "control": "BranchesExceptionsAndSystem",
    "simd_dp": "AdvancedSimd",
}
COLUMNS = [
    "kind", "name", "mnemonic", "class", "feature", "privilege", "mask", "value", "should_be_mask",
    "should_be_value", "reserved", "example", "fields", "precedence_over", "llvm_records", "arm_encoding",
    "alias_of", "condition", "syntax",
]


# Cubes: a set of words given by (mask, value), every word whose masked bits equal the value.

def overlaps(first, second):
    return (first[1] ^ second[1]) & first[0] & second[0] == 0


def contains(outer, inner):
    return inner[0] & outer[0] == outer[0] and inner[1] & outer[0] == outer[1]


def subtract(cube, removed):
    """The words of `cube` outside `removed`, as disjoint cubes."""
    if not overlaps(cube, removed):
        return [cube]
    pieces = []
    mask, value = cube
    for bit in range(WORD_BITS):
        flag = 1 << bit
        if removed[0] & flag and not mask & flag:
            pieces.append((mask | flag, value | (~removed[1] & flag)))
            mask, value = mask | flag, value | (removed[1] & flag)
    return pieces


def subtract_all(cube, removed):
    pieces = [cube]
    for other in removed:
        pieces = [piece for part in pieces for piece in subtract(part, other)]
    return pieces


def hexed(number):
    return f"{number:#010x}"


# Arm's release.

def bits_of(text):
    return text.strip("'")


def feature_expression(node):
    """The feature part of a condition in the list's notation, None where it names no feature."""
    kind = node.get("_type")
    if kind == "AST.Function" and node.get("name") == "IsFeatureImplemented":
        return node["arguments"][0]["value"]
    if kind == "AST.Identifier" and str(node.get("value", "")).startswith("FEAT_"):
        return node["value"]
    if kind == "AST.BinaryOp" and node["op"] in ("&&", "||"):
        left, right = feature_expression(node["left"]), feature_expression(node["right"])
        # A feature that is required only for some field values is no requirement of the encoding as a whole.
        if node["op"] == "||" and (left is None or right is None):
            return None
        if left is None or right is None:
            return left or right
        joiner = " & " if node["op"] == "&&" else " | "
        wrap = lambda part: f"({part})" if (" | " in part and joiner == " & ") or (" & " in part and joiner == " | ") else part
        return wrap(left) + joiner + wrap(right)
    return None


def conjuncts(node):
    if node.get("_type") == "AST.BinaryOp" and node["op"] == "&&":
        return conjuncts(node["left"]) + conjuncts(node["right"])
    return [node]


def exclusion(node):
    """A conjunct `field != 'bits'`, as the field's name and its bits."""
    if node.get("_type") == "AST.BinaryOp" and node["op"] == "!=" and node["left"].get("_type") == "AST.Identifier":
        right = node["right"]
        if right.get("_type") == "Values.Value":
            return node["left"]["value"], bits_of(right["value"])
    return None


def read_arm(path):
    data = json.load(open(path))
    a64 = next(item for item in data["instructions"] if item["name"] == "A64")
    encodings = []

    def walk(node, conditions, groups, fields_above):
        conditions = conditions + [node.get("condition") or {"_type": "AST.Bool", "value": True}]
        fields = dict(fields_above)
        for field in (node.get("encoding") or {}).get("values") or []:
            if field.get("name"):
                fields[field["name"]] = (field["range"]["start"], field["range"]["width"])
        if node.get("_type") == "Instruction.Instruction":
            mask = value = should_mask = should_value = 0
            own_fields = []
            for field in node["encoding"]["values"]:
                start = field["range"]["start"]
                bits, should = bits_of(field["value"]["value"]), bits_of(field["should_be_mask"]["value"])
                for offset in range(len(bits)):
                    flag = 1 << (start + len(bits) - 1 - offset)
                    if should[offset] == "1":
                        should_mask |= flag
                        should_value |= flag if bits[offset] == "1" else 0
                    elif bits[offset] in "01":
                        mask |= flag
                        value |= flag if bits[offset] == "1" else 0
                if field.get("name") and ("x" in bits or "1" in should):
                    own_fields.append((field["name"], start, field["range"]["width"]))
            features = None
            for condition in conditions:
                part = feature_expression(condition)
                if part:
                    features = part if features is None else f"{features} & {part}"
            if features:
                features = normal_feature(features)
            excluded = []
            for condition in conditions:
                for conjunct in conjuncts(condition):
                    found = exclusion(conjunct)
                    if found and found[0] in fields:
                        start, width = fields[found[0]]
                        cube_mask = cube_value = 0
                        for offset, bit in enumerate(found[1]):
                            flag = 1 << (start + width - 1 - offset)
                            if bit in "01":
                                cube_mask |= flag
                                cube_value |= flag if bit == "1" else 0
                        excluded.append((found[0], found[1], (cube_mask, cube_value)))
            literal = next(
                (symbol["value"] for symbol in node["assembly"]["symbols"] if symbol["_type"] == "Instruction.Symbols.Literal"),
                node["name"].split("_")[0],
            )
            encodings.append({
                "name": node["name"],
                "mnemonic": literal.split(".")[0].split(" ")[0].lower(),
                "cube": (mask, value),
                "should": (should_mask, should_value),
                "fields": own_fields,
                "feature": features or "base",
                "excluded": excluded,
                "groups": groups,
            })
            return
        for child in node.get("children") or []:
            if child.get("_type") != "Instruction.InstructionAlias":
                walk(child, conditions, groups + [node.get("name")], fields)

    walk(a64, [], [], {})
    return encodings


# The list as LLVM gave it.

def read_list(path):
    header_lines, rows, columns = [], [], None
    for line in open(path):
        if line.startswith("#"):
            header_lines.append(line.rstrip("\n"))
            continue
        cells = line.rstrip("\n").split("\t")
        if columns is None:
            columns = cells
            continue
        row = dict(zip(columns, cells))
        row.setdefault("arm_encoding", "")
        rows.append(row)
    return header_lines, rows


def cube_of(row):
    return int(row["mask"], 16), int(row["value"], 16)


def reserved_of(row):
    if row["reserved"] in ("-", ""):
        return []
    return [tuple(int(part, 16) for part in pattern.split("/")) for pattern in row["reserved"].split(" ")]


def render_reserved(patterns):
    unique = []
    for pattern in patterns:
        if pattern not in unique:
            unique.append(pattern)
    return " ".join(f"{hexed(mask)}/{hexed(value)}" for mask, value in sorted(unique)) or "-"


def pascal(name):
    return "".join(part[:1].upper() + part[1:].lower() for part in name.replace("-", "_").split("_") if part)


def feature_terms(feature):
    """The terms a feature expression requires all of, each once, LLVM's `+` read as `&` and its extensions as Arm's."""
    terms = []
    for term in feature.replace("+", " & ").split(" & "):
        term = term.strip()
        if term and term not in terms:
            terms.append(term)
    return terms


def normal_feature(feature):
    return " & ".join(feature_terms(feature))


def canonical_feature(feature):
    """A feature expression with `+` read as `&`, each operator's operands flattened, deduplicated and sorted."""
    text = feature.replace("+", " & ").replace("(", " ( ").replace(")", " ) ")
    tokens = text.split()
    position = 0

    def parse(level):
        nonlocal position
        operator = "|" if level == 0 else "&"
        parts = [parse(level + 1) if level == 0 else atom()]
        while position < len(tokens) and tokens[position] == operator:
            position += 1
            parts.append(parse(level + 1) if level == 0 else atom())
        flat = []
        for part in parts:
            flat.extend(part[1] if isinstance(part, tuple) and part[0] == operator else [part])
        unique = sorted({render(part) for part in flat})
        return unique[0] if len(unique) == 1 else (operator, unique)

    def atom():
        nonlocal position
        token = tokens[position]
        position += 1
        if token == "(":
            inner = parse(0)
            position += 1
            return inner
        return token

    def render(part):
        if isinstance(part, tuple):
            return "(" + f" {part[0]} ".join(part[1]) + ")"
        return part

    return render(parse(0)) if tokens else ""


def parse_fields(text):
    fields = []
    for field in filter(None, text.split("; ")):
        name, rest = field.split("@", 1)
        runs, meaning = rest.split("=", 1)
        fields.append([name, [tuple(int(bit) for bit in run.split(":")) for run in runs.split(",")], meaning])
    return fields


def render_fields(fields):
    return "; ".join(f"{name}@{','.join(f'{high}:{low}' for high, low in runs)}={meaning}" for name, runs, meaning in fields)


def run_bits(runs):
    bits = 0
    for high, low in runs:
        for bit in range(low, high + 1):
            bits |= 1 << bit
    return bits


def widen_fields(fields, arm_fields, freed):
    """Widens each field to the Arm field holding it where the bits LLVM fixed and Arm leaves free extend it."""
    changed = []
    for field in fields:
        name, runs, meaning = field
        if len(runs) != 1:
            continue
        high, low = runs[0]
        for arm_name, start, width in arm_fields:
            arm_high, arm_low = start + width - 1, start
            arm_bits = run_bits([(arm_high, arm_low)])
            if arm_low <= low and high <= arm_high and (arm_high, arm_low) != (high, low) and arm_bits & ~run_bits(runs) & ~freed == 0:
                field[1] = [(arm_high, arm_low)]
                changed.append(f"{name} {high}:{low} to {arm_high}:{arm_low}")
    return changed


def narrow_fields(fields, fixed):
    """Takes the bits an encoding fixes out of each field that covers them."""
    changed = []
    for field in fields:
        name, runs, meaning = field
        bits = run_bits(runs)
        if bits & fixed == 0:
            continue
        kept = [bit for bit in range(WORD_BITS - 1, -1, -1) if bits & (1 << bit) and not fixed & (1 << bit)]
        new_runs = []
        for bit in kept:
            if new_runs and new_runs[-1][1] == bit + 1:
                new_runs[-1] = (new_runs[-1][0], bit)
            else:
                new_runs.append((bit, bit))
        field[1] = new_runs
        changed.append(f"{name} loses bits {hexed(bits & fixed)}")
    return [field for field in fields if field[1]], changed


def main(arm_path, llvm_path, list_path, differences_path):
    arm = read_arm(arm_path)
    header_lines, rows = read_list(llvm_path)
    variants = [row for row in rows if row["kind"] == "instruction"]
    aliases = [row for row in rows if row["kind"] == "alias"]
    differences = []

    def note(kind, encoding, detail, reason):
        differences.append((kind, encoding, detail, reason))

    out = []
    used = set()
    renamed = {}
    by_group = collections.defaultdict(collections.Counter)
    # Each LLVM variant belongs to the narrowest Arm encoding that holds all its words.
    home = {}
    for variant in variants:
        holders = [index for index, encoding in enumerate(arm) if contains(encoding["cube"], cube_of(variant))]
        if holders:
            home[variant["name"]] = max(holders, key=lambda index: bin(arm[index]["cube"][0]).count("1"))
    # The rows of each Arm encoding, built from the LLVM variants that share its words.
    for position, encoding in enumerate(arm):
        cube = encoding["cube"]
        holding = [variant for variant in variants if overlaps(cube_of(variant), cube)]
        members = [variant for variant in holding if home.get(variant["name"]) == position]
        equal = [variant for variant in members if cube_of(variant) == cube]
        inside = [variant for variant in members if cube_of(variant) != cube]
        around = [variant for variant in holding if contains(cube_of(variant), cube) and cube_of(variant) != cube]
        others = [other["cube"] for other in arm if other is not encoding and overlaps(other["cube"], cube)]
        row = None
        if equal and not inside:
            variant = equal[0]
            row = dict(variant)
            used.add(variant["name"])
        elif members:
            inside = equal + inside
            row = dict(inside[0])
            names = [variant["name"] for variant in inside]
            for variant in inside:
                used.add(variant["name"])
            row["name"] = NAMES.get(encoding["name"], names[0] if len(inside) == 1 or equal else pascal(encoding["name"]))
            covered = [cube_of(variant) for variant in inside]
            freed = cube[0] ^ covered[0][0] if len(inside) == 1 else 0
            reserved = []
            for variant in inside:
                variant_cube = cube_of(variant)
                extra = variant_cube[0] & ~cube[0]
                for mask, value in reserved_of(variant):
                    reserved.append((mask | extra, value | (variant_cube[1] & extra)))
            # The words of another Arm encoding inside this one are that encoding's, not unallocated ones of this.
            held_inside = [other for other in others if contains(cube, other)]
            for piece in subtract_all(cube, covered + held_inside):
                reserved.append((piece[0] & ~cube[0], piece[1] & ~cube[0]))
            row["reserved"] = render_reserved(reserved)
            row["llvm_records"] = " ".join(variant["llvm_records"] for variant in inside)
            kept_cube = cube
            if encoding["name"] in MERGED_FIELDS:
                row["fields"] = MERGED_FIELDS[encoding["name"]]
                change = "fields " + MERGED_FIELDS[encoding["name"]]
            else:
                fields = parse_fields(row["fields"])
                freed_bits = 0
                for variant in inside:
                    freed_bits |= cube_of(variant)[0] & ~cube[0]
                change = ", ".join(widen_fields(fields, encoding["fields"], freed_bits)) or "fields as LLVM's first record"
                row["fields"] = render_fields(fields)
                # A freed bit no operand field holds stays as LLVM fixes it, so that every word the list's operands
                # make is one of the instruction's; the words Arm's decode makes undefined there stay unallocated.
                held = 0
                for _, runs, _ in fields:
                    held |= run_bits(runs)
                left = freed_bits & ~held
                if left and len(inside) == 1:
                    kept_cube = (cube[0] | left, cube[1] | (cube_of(inside[0])[1] & left))
                    change += f"; keeps LLVM's fixed bits {hexed(left)}, which no operand field holds"
                    reserved = [pattern for pattern in reserved if pattern[0] & left == 0 or pattern[1] & left == kept_cube[1] & left]
                    reserved = [(mask & ~left, value & ~left) for mask, value in reserved if (mask & ~left)]
                    row["reserved"] = render_reserved(reserved)
            row["kept_cube"] = kept_cube
            for variant in inside:
                if variant["name"] != row["name"]:
                    renamed[variant["name"]] = row["name"]
            if len(inside) == 1:
                note("widened", encoding["name"], f"{inside[0]['name']}: fixed bits {inside[0]['mask']}/{inside[0]['value']} to "
                     f"{hexed(cube[0])}/{hexed(cube[1])}; {change}",
                     "LLVM cut operand values out of its fixed bits; Arm leaves them in the encoding, so the list follows "
                     "Arm and marks the words outside LLVM's bits reserved, as the decode makes them undefined")
            else:
                note("merged", encoding["name"], f"{' '.join(names)} into {row['name']}; {change}",
                     "LLVM keeps one record for each value of a field Arm's one encoding has as an operand; the list "
                     "follows Arm")
        elif around:
            variant = min(around, key=lambda candidate: -bin(cube_of(candidate)[0]).count("1"))
            row = dict(variant)
            used.add(variant["name"])
            others_inside = [other for other in arm if other is not encoding and contains(cube_of(variant), other["cube"])]
            if others_inside:
                row["name"] = NAMES.get(encoding["name"], pascal(encoding["name"]))
                renamed.setdefault(variant["name"], row["name"])
            fixed = cube[0] & ~cube_of(variant)[0]
            fields, change = narrow_fields(parse_fields(row["fields"]), fixed)
            row["fields"] = render_fields(fields)
            row["reserved"] = render_reserved([
                pattern for pattern in reserved_of(variant)
                if overlaps((pattern[0] | cube[0], pattern[1] | cube[1]), cube) and pattern[0] & ~cube[0]
            ])
            example = row["example"]
            if example not in ("-", "") and not contains(cube, (ALL_BITS, int(example, 16))):
                row["example"] = "-"
            kind = "split" if others_inside else "narrowed"
            reason = ("LLVM's one record holds several of Arm's encodings; the list follows Arm" if others_inside else
                      "Arm fixes bits that LLVM leaves to an operand (marking values reserved) or to a should-be field; "
                      "the list follows Arm and takes them out of that field")
            note(kind, encoding["name"], f"{variant['name']} to {row['name']} {hexed(cube[0])}/{hexed(cube[1])}; "
                 f"{', '.join(change) or 'fields kept'}", reason)
        elif holding:
            variant = holding[0]
            row = dict(variant)
            used.add(variant["name"])
            row["name"] = NAMES.get(encoding["name"], pascal(encoding["name"]))
            fixed = cube[0] & ~cube_of(variant)[0]
            fields, _ = narrow_fields(parse_fields(row["fields"]), fixed)
            row["fields"] = render_fields(fields)
            row["reserved"] = "-"
            row["example"] = "-"
            note("partial overlap", encoding["name"], " ".join(candidate["name"] for candidate in holding),
                 "Arm's encoding and LLVM's records share only some words (MOPS register rules LLVM writes as separate "
                 "records); the list takes Arm's bits and the first record's operand meanings")
        else:
            row = {column: "" for column in COLUMNS}
            row.update({
                "kind": "instruction",
                "name": NAMES.get(encoding["name"], pascal(encoding["name"])),
                "mnemonic": encoding["mnemonic"],
                "class": "",
                "privilege": "Unprivileged",
                "reserved": "-",
                "example": "-",
                "fields": "; ".join(
                    f"{name}@{start + width - 1}:{start}=encoding field {name} of {encoding['name']}"
                    for name, start, width in encoding["fields"]
                ),
                "precedence_over": "",
                "llvm_records": "-",
                "alias_of": "-",
                "condition": "-",
                "syntax": encoding["mnemonic"] + "".join(f" ${name}" for name, _, _ in encoding["fields"]),
            })
            note("only in Arm's release", encoding["name"], f"{row['name']} {hexed(cube[0])}/{hexed(cube[1])} {encoding['feature']}",
                 "a feature newer than LLVM 23.1.5: no LLVM record, so no operand meaning, example or llvm-mc reserved "
                 "values; class from its Arm group, EL0 behavior unprivileged (INFERRED)")
        # What Arm decides: the fixed and should-be bits, the feature and the values its conditions exclude.
        listed_cube = row.pop("kept_cube", cube)
        row["mask"], row["value"] = hexed(listed_cube[0]), hexed(listed_cube[1])
        listed_should = (int(row["should_be_mask"] or "0", 16), int(row["should_be_value"] or "0", 16))
        if listed_should != encoding["should"]:
            note("should-be bits", encoding["name"], f"{row['name']}: LLVM {hexed(listed_should[0])}/{hexed(listed_should[1])}, "
                 f"Arm {hexed(encoding['should'][0])}/{hexed(encoding['should'][1])}",
                 "Arm marks these bits should-be (constrained unpredictable otherwise), which LLVM leaves undecoded or "
                 "fixes; the list follows Arm")
        row["should_be_mask"], row["should_be_value"] = hexed(encoding["should"][0]), hexed(encoding["should"][1])
        # A field LLVM leaves undecoded where Arm has should-be bits is those bits, not an operand.
        fields, taken = narrow_fields(parse_fields(row["fields"]), encoding["should"][0])
        if taken:
            row["fields"] = render_fields(fields)
        if row["feature"] and canonical_feature(row["feature"]) != canonical_feature(encoding["feature"]):
            llvm_terms = canonical_feature(row["feature"])
            if row["feature"] == "base":
                reason = "LLVM accepts the encoding without a feature (a hint executes as NOP without it); Arm names it"
            elif "+" in row["feature"]:
                reason = "LLVM's subtarget feature turns several Arm features on together; Arm names the one this encoding needs"
            else:
                reason = "LLVM's predicate names other or more features than Arm's condition"
            note("feature", encoding["name"], f"{row['name']}: LLVM {row['feature']}, Arm {encoding['feature']}",
                 reason + "; the list follows Arm")
        row["feature"] = encoding["feature"]
        row["arm_encoding"] = encoding["name"]
        reserved = reserved_of(row)
        llvm_reserved = list(reserved)
        for field, bits, excluded in encoding["excluded"]:
            words = (cube[0] | excluded[0], cube[1] | excluded[1])
            if not overlaps(words, cube):
                continue
            for piece in subtract_all(words, others):
                pattern = (piece[0] & ~cube[0], piece[1] & ~cube[0])
                if pattern[0] and pattern not in reserved:
                    reserved.append(pattern)
                    note("reserved by Arm only", encoding["name"], f"{row['name']}: {field} != '{bits}' gives {hexed(pattern[0])}/{hexed(pattern[1])}",
                         "Arm's encoding condition excludes the value and no other encoding holds it; llvm-mc's probes "
                         "did not find it (they change one field of accepted words); the list follows Arm")
        if llvm_reserved and row["llvm_records"] != "-":
            note("reserved by llvm-mc only", encoding["name"], f"{row['name']}: {render_reserved(llvm_reserved)}",
                 "values the decode pseudocode makes undefined, which Arm's release leaves out; llvm-mc rejects every "
                 "word holding them, so the list keeps them")
        row["reserved"] = render_reserved(reserved)
        by_group[tuple(encoding["groups"])][row["class"]] += 1 if row["class"] else 0
        out.append((row, encoding))
    # Rows Arm alone has take the class of their listed neighbours in Arm's groups.
    for row, encoding in out:
        if row["class"]:
            continue
        groups = encoding["groups"]
        for depth in range(len(groups), 0, -1):
            counts = collections.Counter()
            for key, classes in by_group.items():
                if list(key[:depth]) == groups[:depth]:
                    counts.update({name: count for name, count in classes.items() if name})
            if counts:
                row["class"] = counts.most_common(1)[0][0]
                break
        else:
            row["class"] = CLASS_BY_GROUP.get(groups[1] if len(groups) > 1 else "", "BranchesExceptionsAndSystem")
    for variant in variants:
        if variant["name"] not in used:
            note("only in LLVM", "-", variant["name"], "no Arm encoding holds its words; left out")
    # Each encoding inside another takes precedence over it.
    cubes = [(row["name"], encoding["cube"]) for row, encoding in out]
    for row, encoding in out:
        row["precedence_over"] = " ".join(
            name for name, other in cubes if name != row["name"] and contains(other, encoding["cube"]) and other != encoding["cube"]
        )
    names = collections.Counter(row["name"] for row, _ in out)
    duplicates = [name for name, count in names.items() if count > 1]
    for row, encoding in out:
        if row["name"] in duplicates:
            row["name"] = pascal(encoding["name"])
    assert len({row["name"] for row, _ in out}) == len(out), "every row has a name of its own"
    rows_by_arm = [(row, encoding) for row, encoding in out]
    for alias in aliases:
        alias = dict(alias)
        alias["arm_encoding"] = "-"
        target = alias["alias_of"]
        if target in renamed:
            alias["alias_of"] = renamed[target]
        else:
            alias_cube = cube_of(alias)
            holders = [row for row, encoding in rows_by_arm if overlaps(encoding["cube"], alias_cube)]
            if target not in {row["name"] for row, _ in rows_by_arm} and holders:
                alias["alias_of"] = holders[0]["name"]
        out.append((alias, None))
    sources = [
        "# A64 instruction variants, one per Arm encoding, and the aliases LLVM spells for them.",
        "# Primary source: Arm AARCHMRS_OPENSOURCE_A_profile_FAT-2026-09 (2026-09_rel, BSD-3-Clause, LICENSE-AARCHMRS.txt): one row per A64",
        "# encoding, its fixed and should-be bits, its features, and the operand values its encoding conditions exclude.",
        "# Second source and cross-check: LLVM 23.1.5 (Apache-2.0 WITH LLVM-exception, LICENSE-LLVM.txt), through llvm-tblgen --dump-json:",
        "# the operand meanings, the values llvm-mc rejects (Arm's release leaves the decode pseudocode out), the example word, class and EL0",
        "# behavior; every difference is in arm-llvm-differences.tsv with the reason the list follows the source it does.",
        "# reserved also holds the values Arm's encoding conditions exclude and the words of Arm's encoding outside LLVM's fixed bits.",
        "# Generated by from_arm_release.py; regenerate rather than edit.",
    ]
    kept = [line for line in header_lines if not line.startswith(("# A64 instruction variants", "# Source:", "# Generated by", "# Primary source", "# encoding, its", "# Second source", "# the operand meanings", "# behavior; every", "# reserved also holds"))]
    with open(list_path, "w") as file:
        for line in sources + kept:
            file.write(line + "\n")
        file.write("\t".join(COLUMNS) + "\n")
        for row, _ in out:
            file.write("\t".join(row.get(column, "") or ("-" if column in ("reserved", "example") else "") for column in COLUMNS) + "\n")
    with open(differences_path, "w") as file:
        file.write("# Every difference between Arm's release (primary) and LLVM 23.1.5 (cross-check) in the A64 list, one per line,\n")
        file.write("# with the reason the list follows the source it does. Generated by from_arm_release.py.\n")
        file.write("# kind\tArm encoding\tdetail\treason\n")
        for difference in differences:
            file.write("\t".join(difference) + "\n")
    counts = collections.Counter(kind for kind, *_ in differences)
    print(f"Arm encodings {len(arm)}, LLVM variants {len(variants)}, rows {sum(1 for row, encoding in out if encoding)}")
    for kind, count in sorted(counts.items()):
        print(f"{kind}\t{count}")
    for old, new in sorted(renamed.items()):
        print(f"renamed\t{old}\t{new}")


if __name__ == "__main__":
    main(*sys.argv[1:5])
