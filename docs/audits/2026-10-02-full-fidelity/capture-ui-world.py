#!/usr/bin/env python3
"""Capture PC font-boundary fixtures from prebuilt pc_tour binaries; no Cargo."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--before", type=Path)
parser.add_argument("--after", type=Path)
parser.add_argument("--before-source", default="retained pre-audit debug example; source revision unverified")
parser.add_argument("--after-source", default="combined fidelity fixes; record exact revision when invoking")
parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[3])
args = parser.parse_args()
if not args.before and not args.after:
    parser.error("supply at least one prebuilt binary")
root = args.repo.resolve()
out = root / "docs/screenshots/fidelity-ui-world"
manifest_path = root / "docs/audits/2026-10-02-full-fidelity/ui-world-captures.json"
manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else {}
common = ["02_main_menu", "04_bills_menu", "06_withdraw_popup", "14_item_menu",
          "24_bills_box12", "25_withdrew_thunderstone", "26_league_hof_type", "27_bag_font_bounds"]
for phase, binary, source in [("before", args.before, args.before_source), ("after", args.after, args.after_source)]:
    if binary is None:
        continue
    binary = binary.resolve()
    entry = {"binary": str(binary), "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "source": source, "captures": []}
    for language in ["en", "zh"]:
        with tempfile.TemporaryDirectory(prefix="pokered-ui-world-") as temp:
            subprocess.run([str(binary), temp, language], cwd=root, check=True, timeout=20, stdout=subprocess.PIPE)
            dest = out / (phase + "-" + language)
            dest.mkdir(parents=True, exist_ok=True)
            cases = common
            for case in cases:
                src = Path(temp) / (case + ".png")
                target = dest / src.name
                shutil.copyfile(src, target)
                entry["captures"].append(str(target.relative_to(root)))
    manifest[phase] = entry
manifest_path.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
print(json.dumps({key: len(value["captures"]) for key, value in manifest.items()}))
