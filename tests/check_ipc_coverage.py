"""Fail when a recovered business command is absent from the Tauri handler."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
catalog = json.loads((ROOT / "analysis" / "frontend-ipc.json").read_text(encoding="utf-8"))
required = {
    command
    for commands in catalog.values()
    for command in commands
    if not command.startswith("plugin:")
}
main_rs = (ROOT / "src-tauri" / "src" / "main.rs").read_text(encoding="utf-8")
registered = set(re.findall(r"(?:\w+::)?([a-z][a-z0-9_]*)\s*(?:,|\])", main_rs))
missing = sorted(required - registered)

stubbed = sorted(required & set(re.findall(r"compat::([a-z][a-z0-9_]*)", main_rs)))

print(f"business commands: {len(required)}")
print(f"registered: {len(required & registered)}")
if missing:
    raise SystemExit("missing commands: " + ", ".join(missing))
print("missing: []")
print(f"placeholder implementations: {len(stubbed)}")
if stubbed:
    raise SystemExit("placeholder commands: " + ", ".join(stubbed))
