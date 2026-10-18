"""Validate emitted CND files against the pinned cnd-sdk checkout.

Usage: ``python -I sdk_gate.py <dir>`` with ``CND_SDK_DIR`` set.

Prints ``FILE: rule: message`` for every failure and exits 1 if any.
"""

import json
import os
import sys
from pathlib import Path

import jsonschema

from cnd.core.cnd import Cnd
from cnd.core.hashing import content_hash, node_hash
from cnd.core.validate import validate

DROPPED = ("id", "built_at")


def strip(doc):
    return {k: v for k, v in doc.items() if k not in DROPPED}


def check(path, schema, sdk):
    name = path.name
    failures = []
    raw = path.read_text(encoding="utf-8")
    emitted = json.loads(raw)

    for err in jsonschema.Draft202012Validator(schema).iter_errors(emitted):
        where = "/".join(str(p) for p in err.absolute_path) or "<root>"
        failures.append(f"{name}: schema: {where}: {err.message}")

    try:
        cnd = Cnd.model_validate_json(raw)
    except Exception as err:  # pydantic.ValidationError and friends
        failures.append(f"{name}: shape: {err}")
        return failures

    for violation in validate(cnd):
        failures.append(f"{name}: invariants: {violation}")

    written = json.loads(cnd.to_json())
    if strip(written) != strip(emitted):
        failures.append(f"{name}: reference-writer: {first_diff(strip(written), strip(emitted), '')}")

    try:
        content_hash(cnd)
        for visit in cnd.iter_all():
            node_hash(visit.node)
    except Exception as err:
        failures.append(f"{name}: hashes: {err!r}")
    return failures


def first_diff(writer, emitted, path):
    """Locate the first difference, for a readable message."""
    if isinstance(writer, dict) and isinstance(emitted, dict):
        for key in sorted(set(writer) | set(emitted)):
            here = f"{path}/{key}"
            if key not in emitted:
                return f"{here} is written by the reference writer but not emitted"
            if key not in writer:
                return f"{here} is emitted but the reference writer drops it"
            if writer[key] != emitted[key]:
                return first_diff(writer[key], emitted[key], here)
    if isinstance(writer, list) and isinstance(emitted, list):
        if len(writer) != len(emitted):
            return f"{path} has {len(emitted)} items emitted, {len(writer)} by the reference writer"
        for i, (a, b) in enumerate(zip(writer, emitted)):
            if a != b:
                return first_diff(a, b, f"{path}/{i}")
    return f"{path}: emitted {emitted!r}, reference writer {writer!r}"


def main():
    directory = Path(sys.argv[1])
    sdk = Path(os.environ["CND_SDK_DIR"])
    schema = json.loads((sdk / "schema" / "cnd.schema.json").read_text(encoding="utf-8"))
    failures = []
    files = sorted(directory.glob("*.cnd"))
    if not files:
        failures.append(f"{directory}: no *.cnd files to validate")
    for path in files:
        failures.extend(check(path, schema, sdk))

    # Vendored package: byte-equal to the SDK's copy, once the examples ship one.
    vendored = Path(__file__).resolve().parent.parent / "examples" / "vendor" / "cnd.typ"
    if vendored.exists():
        original = sdk / "src" / "cnd" / "typst" / "cnd.typ"
        if vendored.read_bytes() != original.read_bytes():
            failures.append(f"{vendored.name}: vendored: differs from {original}")

    for line in failures:
        print(line)
    print(f"sdk_gate: {len(files)} files, {len(failures)} failures")
    sys.exit(1 if failures else 0)


main()
