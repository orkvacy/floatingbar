"""The GUI executable's hook mode must fall back through stdio without launching UI."""
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
profile = root / "src-tauri/target/floatingbar-test-helper-offline"
assert not (profile / "permission-hook.dat").exists(), "Use a fresh offline test profile"
result = subprocess.run(
    [str(root / "src-tauri/target/release/floatingbar.exe"), "--permission-hook", "--test-profile", str(profile)],
    input=json.dumps({"hook_event_name": "PermissionRequest", "tool_name": "Bash", "tool_input": {"command": "echo fixture"}}),
    capture_output=True, text=True, encoding="utf-8", timeout=5,
    creationflags=subprocess.CREATE_NO_WINDOW,
)
assert result.returncode == 0
assert json.loads(result.stdout) == {}, "Offline helper must return no approval"
assert not profile.exists(), "Offline helper must not initialize application UI/data"
print("PASS: GUI hook helper returns {} through stdout while offline; no UI or approval")
