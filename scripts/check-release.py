"""Check working-tree or staged release inputs without printing secret values."""
import argparse
import base64
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, stderr=subprocess.DEVNULL)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--staged", action="store_true")
    parser.add_argument("--artifact", type=Path, action="append", default=[])
    args = parser.parse_args()
    # Read historical values privately; never pass credentials as CLI arguments.
    historical = set()
    for source in ("src-tauri/src/discord.rs", "src/server/discord-bridge.cjs"):
        try:
            old = git("show", f"HEAD:{source}").decode("utf-8")
        except (subprocess.CalledProcessError, UnicodeDecodeError):
            continue
        historical.update(re.findall(r'CLIENT_SECRET[^\n=]*=\s*["\']([^"\']+)["\']', old))
    needles = set()
    for value in historical:
        if len(value) > 12 and not value.startswith("process."):
            raw = value.encode()
            needles.update((raw, value.encode("utf-16-le"), base64.b64encode(raw)))
    files = git("ls-files", "-z", "--cached")
    if not args.staged:
        files += git("ls-files", "-z", "--others", "--exclude-standard")
    failures = []
    private_paths = [str(ROOT), os.environ.get("USERPROFILE", "")]
    patterns = [p.encode().lower() for value in private_paths if value for p in (value, value.replace("\\", "/"), value.replace("\\", "\\\\"))]
    forbidden = re.compile(r'(^|/)(\.env(?:\..*)?|client_secret[^/]*\.json|credentials[^/]*\.json|discord_token\.json|discord_oauth\.json|(?:discord-client|discord-token|google-calendar)\.(?:dat|tmp)|codex-clipboard-[^/]*\.png)$', re.I)
    literal_secret = re.compile(rb'(?:client_secret|CLIENT_SECRET)\s*(?::[^=\n]+)?\s*[:=]\s*["\']([A-Za-z0-9_\-]{20,})["\']')
    token_pattern = re.compile(rb'(?:ya29\.[A-Za-z0-9_\-]{25,}|gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,})')

    def scan(name, data, artifact=False):
        if any(needle in data for needle in needles):
            failures.append((name, "historical Discord secret present"))
        if not artifact:
            if any(p in data.lower() for p in patterns):
                failures.append((name, "private absolute workspace/profile path"))
            if literal_secret.search(data):
                failures.append((name, "literal client secret"))
            if token_pattern.search(data):
                failures.append((name, "Google/GitHub token pattern"))
            if (b"-----BEGIN " + b"PRIVATE KEY-----") in data:
                failures.append((name, "private key"))

    for name in sorted(set(files.decode("utf-8").split("\0")) - {""}):
        if forbidden.search(name) and Path(name).name != ".env.example":
            failures.append((name, "private runtime/credential filename"))
        try:
            data = git("show", f":{name}") if args.staged else (ROOT / name).read_bytes()
        except (FileNotFoundError, subprocess.CalledProcessError):
            continue
        scan(name, data)
    for artifact in args.artifact:
        scan(artifact.name, artifact.read_bytes(), artifact=True)
    for name, rule in failures:
        print(f"FAIL {name}: {rule}")
    print(f"{'FAIL' if failures else 'PASS'}: {'staged index' if args.staged else 'working release inputs'}; {len(args.artifact)} artifact(s).")
    print("Historical exposure still requires rotating the old Discord secret. Images require visual review.")
    return bool(failures)


if __name__ == "__main__":
    sys.exit(main())
