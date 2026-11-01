"""Smoke-test a built Windows app in a new isolated profile, without accounts."""
import json
from pathlib import Path
import subprocess
import time
import uuid

root = Path(__file__).resolve().parents[1]
exe = root / "src-tauri/target/release/floatingbar.exe"
profile = root / "src-tauri/target" / f"floatingbar-test-files-{uuid.uuid4().hex}"
settings = profile / "claude/settings.json"
settings.parent.mkdir(parents=True)
original = {
    "env": {"FIXTURE": "preserved"},
    "hooks": {
        "Stop": [{"hooks": [{"type": "command", "command": "fixture-never-executed"}]}],
        "PermissionRequest": [{"matcher": "Bash", "hooks": [
            {"type": "http", "url": "http://127.0.0.1:47821/permission"},
            {"type": "http", "url": "https://example.invalid/unrelated"},
        ]}],
    },
}
settings.write_text(json.dumps(original), encoding="utf-8")
process = subprocess.Popen([str(exe), "--test-profile", str(profile)], creationflags=subprocess.CREATE_NO_WINDOW)
try:
    deadline = time.monotonic() + 15
    backup = settings.with_suffix(".json.floatingbar-backup")
    while True:
        migrated = json.loads(settings.read_text(encoding="utf-8"))
        if backup.exists() and migrated["hooks"]["PermissionRequest"][0]["hooks"][0].get("type") == "command":
            break
        assert process.poll() is None, "Application exited before migration"
        assert time.monotonic() < deadline, "Migration did not finish"
        time.sleep(0.1)
    assert json.loads(backup.read_text(encoding="utf-8")) == original
    assert migrated["env"] == original["env"]
    assert migrated["hooks"]["Stop"] == original["hooks"]["Stop"]
    group = migrated["hooks"]["PermissionRequest"][0]
    assert group["matcher"] == "Bash"
    assert group["hooks"][1] == original["hooks"]["PermissionRequest"][0]["hooks"][1]
    hook = group["hooks"][0]
    assert Path(hook["command"]) == exe
    assert hook["args"] == ["--permission-hook", "--test-profile", str(profile)]
    assert (profile / "permission-hook.dat").is_file()
    # An invalid event reaches the live authenticated bridge and safely falls back.
    result = subprocess.run(
        [str(exe), *hook["args"]], input='{"hook_event_name":"Other"}',
        capture_output=True, text=True, timeout=5, creationflags=subprocess.CREATE_NO_WINDOW,
    )
    assert result.returncode == 0 and json.loads(result.stdout) == {}
    assert not result.stderr
    print("PASS: isolated startup, DPAPI/helper fallback, migration, backup, and preservation of unrelated settings")
finally:
    process.terminate()
    process.wait(timeout=10)
