#!/usr/bin/env python3
"""Assemble the self-contained web demo (demo/mascii-demo.html).

Inlines into demo/template.html:
- the wasm-bindgen JS glue and the .wasm binary (base64) from wasm/pkg
  (build first: cd wasm && wasm-pack build --target web --out-dir pkg)
- a JuliaMono woff2 subset (base64) covering ASCII + the math ranges
  (pass the subset with --font; create it with fontTools, see docs/editors.md)
- example formulas extracted from docs/examples.md (always in sync with
  the roundtrip corpus)

Usage:
    python3 tools/build_web_demo.py --font JuliaMono-sub.woff2
        [-o demo/mascii-demo.html] [--fragment out.html]

`--fragment` additionally writes a body-only fragment (no <!doctype>/<html>
wrapper) for hosts that provide their own document shell.
"""

import argparse
import base64
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# name in docs/examples.md -> button label
EXAMPLE_PICKS = {
    "gaussian": "ガウス積分",
    "cardano": "カルダノの公式",
    "rotation": "回転行列",
    "variance": "分散",
    "cancel-simple": "約分(打ち消し)",
    "continued-fraction": "連分数",
}


def extract_examples() -> dict:
    md = (ROOT / "docs/examples.md").read_text()
    found = {}
    for m in re.finditer(r"### (\S+)\n\n```\n(.*?)\n```", md, re.S):
        name, aa = m.group(1), m.group(2)
        if name in EXAMPLE_PICKS:
            found[EXAMPLE_PICKS[name]] = aa
    missing = set(EXAMPLE_PICKS) - {
        k for k in EXAMPLE_PICKS if EXAMPLE_PICKS[k] in found
    }
    if missing:
        raise SystemExit(f"examples not found in docs/examples.md: {missing}")
    return found


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--font", required=True, help="JuliaMono woff2 subset")
    ap.add_argument("-o", "--output", default=str(ROOT / "demo/mascii-demo.html"))
    ap.add_argument("--fragment", default=None,
                    help="also write a body-only fragment to this path")
    args = ap.parse_args()

    pkg = ROOT / "wasm/pkg"
    glue = (pkg / "mascii_wasm.js").read_text()
    # The inline module never imports the glue, so drop its export statement
    # and expose the two names it would have exported.
    glue = glue.replace(
        "export { initSync, __wbg_init as default };",
        "// (inlined: initSync / __wbg_init are in scope)",
    )
    glue = re.sub(r"^export (function|class)", r"\1", glue, flags=re.M)

    wasm_b64 = base64.b64encode((pkg / "mascii_wasm_bg.wasm").read_bytes()).decode()
    font_b64 = base64.b64encode(Path(args.font).read_bytes()).decode()

    fragment = (
        (ROOT / "demo/template.html")
        .read_text()
        .replace("{{GLUE}}", glue)
        .replace("{{WASM_B64}}", wasm_b64)
        .replace("{{FONT_B64}}", font_b64)
        .replace("{{EXAMPLES_JSON}}", json.dumps(extract_examples(), ensure_ascii=False))
    )

    full = "<!doctype html>\n<html lang=\"ja\">\n<head>\n</head>\n<body>\n" + fragment + "\n</body>\n</html>\n"
    Path(args.output).write_text(full)
    print(f"wrote {args.output} ({len(full) // 1024} KiB)")
    if args.fragment:
        Path(args.fragment).write_text(fragment)
        print(f"wrote {args.fragment} ({len(fragment) // 1024} KiB)")


if __name__ == "__main__":
    main()
