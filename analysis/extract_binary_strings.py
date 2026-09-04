"""Extract useful UTF-8/ASCII strings from the original Tauri executable."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


ASCII = re.compile(rb"[\x20-\x7e]{4,}")
UTF8_RUN = re.compile(rb"(?:[\x20-\x7e]|[\xc2-\xf4][\x80-\xbf]{1,3}){4,}")
INTERESTING = re.compile(
    r"tauri|invoke|command|ping|tcp|udp|dns|http|route|port|serial|ffmpeg|"
    r"network|adapter|traffic|packet|ip|mac|wifi|system|monitor|speed|test|"
    r"网络|测试|工具|端口|路由|流量|网卡|系统|串口|视频|截图|扫描|检测|地址|延迟",
    re.IGNORECASE,
)


def extract(blob: bytes) -> list[dict[str, object]]:
    found: dict[tuple[int, str], None] = {}
    for pattern in (ASCII, UTF8_RUN):
        for match in pattern.finditer(blob):
            value = match.group().decode("utf-8", errors="ignore").strip()
            if len(value) < 4 or not INTERESTING.search(value):
                continue
            if len(value) > 4000:
                value = value[:4000] + "…"
            found[(match.start(), value)] = None
    return [{"offset": offset, "text": value} for offset, value in sorted(found)]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    rows = extract(args.binary.read_bytes())
    args.output.write_text(json.dumps(rows, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"wrote {len(rows)} strings to {args.output}")


if __name__ == "__main__":
    main()
