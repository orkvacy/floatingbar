"""Stage the current version's installers and SHA256SUMS for manual publication."""
import hashlib
import json
from pathlib import Path
import re
import shutil

root = Path(__file__).resolve().parents[1]
version = json.loads((root / "package.json").read_text(encoding="utf-8"))["version"]
assert re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version)
bundle = root / "src-tauri/target/release/bundle"
files = [bundle / "nsis" / f"FloatingBar_{version}_x64-setup.exe", bundle / "msi" / f"FloatingBar_{version}_x64_en-US.msi"]
assert all(p.is_file() for p in files), "Build both current-version installers first"
out = root / "src-tauri/target/release/release-assets" / version
out.mkdir(parents=True, exist_ok=True)
lines = []
for source in files:
    destination = out / source.name
    shutil.copy2(source, destination)
    lines.append(f"{hashlib.sha256(destination.read_bytes()).hexdigest()}  {destination.name}")
(out / "SHA256SUMS.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
print(f"PASS: staged {len(files)} installers and SHA256SUMS for {version}")
