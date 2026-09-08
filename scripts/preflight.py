#!/usr/bin/env python3
"""Offline repository checks. These do NOT compile Rust or run the Rust tests."""
import argparse
import ast
import hashlib
import json
import re
import string
import struct
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUST_TOKEN = re.compile(r'//[^\n]*|/\*[\s\S]*?\*/|b?"(?:\\[\s\S]|[^"\\])*"|b?\'(?:\\(?:u\{[0-9A-Fa-f]+\}|x[0-9A-Fa-f]{2}|[\s\S])|[^\'\\])\'|[{}\[\]()]')
FORMAT_MACRO = re.compile(r'\b(?:write|writeln|format|panic)!\s*\(')
STRING_LITERAL = re.compile(r'"(?:\\[\s\S]|[^"\\])*"')


def check_rust_lexical(path):
    text = path.read_text(encoding="utf-8")
    stack = []
    pairs = {"}": "{", "]": "[", ")": "("}
    for match in RUST_TOKEN.finditer(text):
        token = match.group()
        if token in "{[(" and len(token) == 1:
            stack.append(token)
        elif token in "}])" and len(token) == 1:
            assert stack and stack.pop() == pairs[token], f"unbalanced delimiter in {path}:{text[:match.start()].count(chr(10)) + 1}"
    assert not stack, f"unclosed delimiters in {path}"
    checked_formats = 0
    for macro in FORMAT_MACRO.finditer(text):
        literal = STRING_LITERAL.search(text, macro.end())
        if not literal:
            continue
        raw = literal.group()
        # Standard format strings here are compatible with Python literal
        # decoding; Rust-only unicode scalar escapes are handled explicitly.
        raw = re.sub(r'(?<!\\)\\u\{([0-9a-fA-F]+)\}', lambda m: chr(int(m[1], 16)), raw)
        try:
            value = ast.literal_eval(raw)
            list(string.Formatter().parse(value))
        except (ValueError, SyntaxError) as error:
            raise AssertionError(f"invalid literal/format braces in {path}:{text[:literal.start()].count(chr(10)) + 1}: {error}") from error
        checked_formats += 1
    return checked_formats


def independent_matches(data):
    """Reference byte matcher used to check fixture expectations, not Rust code."""
    fixed = [bytes.fromhex(value) for value in ["0f05", "0f34", "cd80", "0f01ef", "0f31", "0f01f9"]]
    count = sum(sum(data.startswith(pattern, offset) for offset in range(len(data))) for pattern in fixed)
    for offset in range(max(0, len(data) - 2)):
        head = data[offset:offset + 2]
        modrm = data[offset + 2]
        if modrm >> 6 != 3 and ((head == b"\x0f\xae" and (modrm >> 3) & 7 == 5) or (head == b"\x0f\xc7" and (modrm >> 3) & 7 == 3)):
            count += 1
    return count


def fixture_payload(data, name, input_format):
    if input_format == "raw":
        return data
    assert data[:7] == b"\x7fELF\x02\x01\x01", name
    kind = struct.unpack_from("<H", data, 16)[0]
    if kind == 1:
        table = struct.unpack_from("<Q", data, 40)[0]
        entry_size, count = struct.unpack_from("<HH", data, 58)
        result = bytearray()
        for index in range(count):
            entry = table + index * entry_size
            flags = struct.unpack_from("<Q", data, entry + 8)[0]
            offset, size = struct.unpack_from("<QQ", data, entry + 24)
            if flags & 4:
                result.extend(data[offset:offset + size])
        return bytes(result)
    table = struct.unpack_from("<Q", data, 32)[0]
    entry_size, count = struct.unpack_from("<HH", data, 54)
    result = bytearray()
    for index in range(count):
        entry = table + index * entry_size
        kind, flags = struct.unpack_from("<II", data, entry)
        offset = struct.unpack_from("<Q", data, entry + 8)[0]
        size = struct.unpack_from("<Q", data, entry + 32)[0]
        if kind == 1 and flags & 1:
            assert offset + size <= len(data)
            result.extend(data[offset:offset + size])
    return bytes(result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json-output", type=Path)
    args = parser.parse_args()
    checks = []
    all_toml = list(ROOT.rglob("*.toml"))
    for path in all_toml:
        if "target" not in path.parts:
            tomllib.loads(path.read_text(encoding="utf-8"))
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    assert {package["name"] for package in lock["package"]} == {"gulag", "synapse", "reactor"}
    assert all("source" not in package for package in lock["package"])
    assert all(package["version"] == "0.2.0" for package in lock["package"])
    checks.append({"name": "toml-and-local-only-lockfile", "status": "passed"})
    rust_files = [path for path in ROOT.rglob("*.rs") if "target" not in path.parts]
    formats = sum(check_rust_lexical(path) for path in rust_files)
    for path in rust_files:
        text = path.read_text(encoding="utf-8")
        for module in re.findall(r'^(?:pub )?mod (\w+);', text, re.M):
            assert (path.parent / (module + '.rs')).exists(), (path, module)
        for included in re.findall(r'include_(?:str|bytes)!\("([^"\n]+)"\)', text):
            assert (path.parent / included).is_file(), (path, included)
    for path in rust_files:
        text = path.read_text(encoding="utf-8")
        assert not re.search(r'\b(?:todo!|unimplemented!|TODO|FIXME)', text), path
    checks.append({"name": "rust-delimiters-and-format-braces-only", "status": "passed", "files": len(rust_files), "format_strings": formats, "is_compilation": False})
    for path in (ROOT / "scripts").glob("*.py"):
        ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    checks.append({"name": "python-syntax", "status": "passed"})
    manifest = json.loads((ROOT / "fixtures/manifest.json").read_text(encoding="utf-8"))
    reference_cases = 0
    for name, entry in manifest.items():
        data = (ROOT / "fixtures" / name).read_bytes()
        assert len(data) == entry["bytes"]
        assert hashlib.sha256(data).hexdigest() == entry["sha256"]
        if entry["expected_matches"] is not None:
            actual = independent_matches(fixture_payload(data, name, entry["input_format"]))
            assert actual == entry["expected_matches"], (name, actual, entry)
            reference_cases += 1
    checks.append({"name": "fixture-hashes-and-independent-expectations", "status": "passed", "fixtures": len(manifest), "reference_cases": reference_cases, "tests_rust_implementation": False})
    broken_links = []
    link_count = 0
    for path in ROOT.rglob("*.md"):
        if "target" in path.parts:
            continue
        for destination in re.findall(r'(?<!!)\[[^\]]+\]\(([^\s)]+)\)', path.read_text(encoding="utf-8")):
            if re.match(r'^[a-zA-Z]+:', destination) or destination.startswith('#'):
                continue
            destination = destination.split('#', 1)[0]
            if not destination:
                continue
            link_count += 1
            if not (path.parent / destination).exists():
                broken_links.append((str(path.relative_to(ROOT)), destination))
    assert not broken_links, broken_links
    checks.append({"name": "relative-documentation-links", "status": "passed", "links": link_count})
    result = {"scope": "offline static repository checks, NOT Rust compilation or CLI execution", "checks": checks, "rust_test_functions_authored": sum(len(re.findall(r'#\[test\]', path.read_text(encoding="utf-8"))) for path in rust_files), "python_e2e_methods_authored": len(re.findall(r'^    def test_', (ROOT / "scripts/e2e.py").read_text(encoding="utf-8"), re.M))}
    encoded = json.dumps(result, indent=2, ensure_ascii=False) + "\n"
    if args.json_output:
        args.json_output.parent.mkdir(parents=True, exist_ok=True)
        args.json_output.write_text(encoded, encoding="utf-8")
    print(encoded)
    return 0


if __name__ == "__main__":
    sys.exit(main())
