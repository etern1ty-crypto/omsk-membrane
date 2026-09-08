#!/usr/bin/env python3
"""Generate deterministic, NON-EXECUTED scanner fixtures using Python stdlib."""
import argparse
import hashlib
import json
import struct
from pathlib import Path


RULE_PAYLOADS = {
    "syscall": bytes.fromhex("0f05"),
    "sysenter": bytes.fromhex("0f34"),
    "int80": bytes.fromhex("cd80"),
    "wrpkru": bytes.fromhex("0f01ef"),
    "xrstor": bytes.fromhex("0fae28"),
    "xrstors": bytes.fromhex("0fc718"),
    "rdtsc": bytes.fromhex("0f31"),
    "rdtscp": bytes.fromhex("0f01f9"),
}


def elf(payload: bytes, executable: bool = True) -> bytes:
    ident = b"\x7fELF\x02\x01\x01" + bytes(9)
    header = struct.pack("<16sHHIQQQIHHHHHH", ident, 3, 62, 1, 0x400080, 64, 0, 0, 64, 56, 1, 0, 0, 0)
    segment = struct.pack("<IIQQQQQQ", 1, 5 if executable else 4, 128, 0x400080, 0x400080, len(payload), len(payload), 1)
    return header + segment + bytes(8) + payload


def elf_object(payload: bytes) -> bytes:
    ident = b"\x7fELF\x02\x01\x01" + bytes(9)
    header = struct.pack("<16sHHIQQQIHHHHHH", ident, 1, 62, 1, 0, 0, 64, 0, 64, 0, 0, 64, 2, 0)
    null_section = bytes(64)
    text_section = struct.pack("<IIQQQQIIQQ", 0, 1, 6, 0, 192, len(payload), 0, 0, 1, 0)
    return header + null_section + text_section + payload


def generate(output: Path) -> dict:
    output.mkdir(parents=True, exist_ok=True)
    all_rules = b"\x90".join(RULE_PAYLOADS.values())
    fixtures = {
        "clean.bin": (bytes.fromhex("9090c3"), "raw", 0, 0),
        "syscall.bin": (bytes.fromhex("900f05c3"), "raw", 1, 1),
        "all-rules.bin": (all_rules, "raw", 1, 8),
        "immediate.bin": (bytes.fromhex("b80f050000c3"), "raw", 1, 1),
        "clean.elf": (elf(bytes.fromhex("9090c3")), "auto", 0, 0),
        "violation.elf": (elf(bytes.fromhex("900f05c3")), "auto", 1, 1),
        "nonexec.elf": (elf(bytes.fromhex("0f05"), False), "auto", 2, None),
        "data-only-pattern.elf": (elf(bytes.fromhex("90c3")) + bytes.fromhex("0f05"), "auto", 0, 0),
        "violation.o": (elf_object(bytes.fromhex("0f05c3")), "auto", 1, 1),
        "truncated.elf": (b"\x7fELF\x02\x01", "auto", 2, None),
        "empty.bin": (b"", "raw", 2, None),
        "unknown.bin": (b"not an ELF artifact", "auto", 2, None),
    }
    manifest = {}
    for name, (payload, input_format, exit_code, matches) in fixtures.items():
        (output / name).write_bytes(payload)
        manifest[name] = {
            "bytes": len(payload),
            "sha256": hashlib.sha256(payload).hexdigest(),
            "input_format": input_format,
            "expected_exit": exit_code,
            "expected_matches": matches,
        }
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=Path(__file__).resolve().parents[1] / "fixtures")
    args = parser.parse_args()
    manifest = generate(args.out)
    print(json.dumps({"fixtures": len(manifest), "destination": str(args.out)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
