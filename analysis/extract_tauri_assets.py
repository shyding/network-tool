"""Carve Brotli-compressed Tauri 2 frontend assets from a release executable.

Tauri's generated Rust code stores each asset key next to its compressed byte
slice.  Walking one Brotli byte at a time lets us stop exactly at the end of the
stream even when linker metadata follows it.
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

import brotli


ASSET_KEY = re.compile(
    rb"/(?:assets/[A-Za-z0-9_.-]+|index\.html|favicon\.svg|tauri\.svg|vite\.svg)"
)


def decompress_prefix(blob: bytes) -> tuple[bytes, int] | None:
    decoder = brotli.Decompressor()
    output = bytearray()
    try:
        for index, value in enumerate(blob, 1):
            output.extend(decoder.process(bytes((value,))))
            if decoder.is_finished():
                return bytes(output), index
    except brotli.error:
        return None
    return None


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    binary = args.binary.read_bytes()
    matches = list(ASSET_KEY.finditer(binary))
    manifest: list[dict[str, object]] = []
    args.output.mkdir(parents=True, exist_ok=True)

    for index, match in enumerate(matches):
        key = match.group().decode("ascii")
        upper = matches[index + 1].start() if index + 1 < len(matches) else min(len(binary), match.end() + 8_000_000)
        result = decompress_prefix(binary[match.end() : upper])
        if result is None:
            continue
        content, compressed_size = result
        destination = args.output / key.removeprefix("/")
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(content)
        manifest.append(
            {
                "key": key,
                "offset": match.start(),
                "compressed_size": compressed_size,
                "size": len(content),
                "output": str(destination.relative_to(args.output)),
            }
        )

    manifest_path = args.output / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"extracted {len(manifest)} assets to {args.output}")


if __name__ == "__main__":
    main()
