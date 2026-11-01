"""Validate release version consistency, frontend policy, and public asset links."""
import json
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]
package = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))
version = package["version"]
assert re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version), "Invalid release version"
config = json.loads((ROOT / "src-tauri/tauri.conf.json").read_text(encoding="utf-8"))
cargo = tomllib.loads((ROOT / "src-tauri/Cargo.toml").read_text(encoding="utf-8"))
lock = tomllib.loads((ROOT / "src-tauri/Cargo.lock").read_text(encoding="utf-8"))
assert config["version"] == cargo["package"]["version"] == version
assert next(p for p in lock["package"] if p["name"] == "floatingbar")["version"] == version
wix_version = config.get("bundle", {}).get("windows", {}).get("wix", {}).get("version")
if "-" in version:
    assert wix_version and re.fullmatch(r"\d+\.\d+\.\d+(?:\.\d+)?", wix_version), "MSI candidates require an explicit numeric version"
if wix_version:
    fields = [int(v) for v in wix_version.split(".")]
    assert len(fields) in (3, 4) and all(0 <= v <= limit for v, limit in zip(fields, [255, 255, 65535, 65535]))
    assert fields[:3] == [int(v) for v in version.split("-")[0].split(".")], "MSI base version must match the app"
csp = config["app"]["security"]["csp"]
assert isinstance(csp, dict), "Release CSP must be explicit"
assert "'unsafe-eval'" not in csp["script-src"] and "'unsafe-inline'" not in csp["script-src"]
assert csp["object-src"] == "'none'" and csp["base-uri"] == "'none'"
assert "https:" not in csp["connect-src"].split(), "Frontend must use scoped IPC, not arbitrary network access"
for filename in ["README.md", "README.id.md", "docs/RELEASING.md", "docs/AI-PERMISSIONS.md", "docs/RELEASE-READINESS.md", f"docs/releases/v{version}.md"]:
    path = ROOT / filename
    for match in re.findall(r'\]\(([^)]+)\)|src="([^"]+)"', path.read_text(encoding="utf-8")):
        target = next(v for v in match if v)
        if "://" not in target and not target.startswith("#"):
            assert (path.parent / target.split("#")[0]).exists(), f"Missing link in {filename}: {target}"
print(f"PASS: version {version}, release CSP, and local documentation links")
