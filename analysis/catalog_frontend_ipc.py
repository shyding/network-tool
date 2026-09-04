"""Catalog Tauri invoke calls used by each recovered frontend module."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).with_name("tauri-assets") / "assets"
IMPORT = re.compile(r'import\{([^}]+)\}from"\./index-DNL4tSY9\.js"')


def invoke_alias(source: str) -> str | None:
    match = IMPORT.search(source[:5000])
    if not match:
        return None
    for item in match.group(1).split(","):
        parts = item.strip().split(" as ")
        if parts[0] == "t":
            return parts[-1]
    return None


def main() -> None:
    catalog: dict[str, list[str]] = {}
    contexts: dict[str, list[dict[str, str]]] = {}
    for path in sorted(ROOT.glob("*.js")):
        source = path.read_text(encoding="utf-8", errors="ignore")
        alias = invoke_alias(source)
        if not alias:
            continue
        direct_matches = list(re.finditer(rf'(?<![\w$]){re.escape(alias)}\("([^"]+)"', source))
        # Some pages pass invoke through a busy/error wrapper and immediately call
        # the returned function, e.g. wrap(invoke)("firewall_set_all", ...).
        wrapped_matches = list(re.finditer(r'\)\("([a-z][a-z0-9_]*_[a-z0-9_]+)"', source))
        mapped_matches = list(re.finditer(r'"(net_svc_stop_[a-z]+)"', source))
        matches = direct_matches + wrapped_matches + mapped_matches
        calls = sorted({match.group(1) for match in matches})
        if calls:
            catalog[path.name] = calls
            contexts[path.name] = []
            for match in sorted(matches, key=lambda item: item.start()):
                contexts[path.name].append({
                    "command": match.group(1),
                    "context": source[max(0, match.start() - 180):match.start() + 520],
                })

    main_source = (ROOT / "index-DNL4tSY9.js").read_text(encoding="utf-8", errors="ignore")
    direct = set(re.findall(r'(?<![\w$])(?:za|ot)\("([^"]+)"', main_source))
    catalog["index-DNL4tSY9.js"] = sorted(direct)
    contexts["index-DNL4tSY9.js"] = [
        {
            "command": match.group(1),
            "context": main_source[max(0, match.start() - 180):match.start() + 520],
        }
        for match in re.finditer(r'(?<![\w$])(?:za|ot)\("([^"]+)"', main_source)
    ]
    output = Path(__file__).with_name("frontend-ipc.json")
    output.write_text(json.dumps(catalog, ensure_ascii=False, indent=2), encoding="utf-8")
    context_output = Path(__file__).with_name("frontend-ipc-contexts.json")
    context_output.write_text(json.dumps(contexts, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"wrote {sum(map(len, catalog.values()))} call sites across {len(catalog)} modules")


if __name__ == "__main__":
    main()
