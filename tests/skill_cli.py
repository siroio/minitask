"""Run the skill's PowerShell example and CLI contracts against isolated data.

Usage: python tests/skill_cli.py [path/to/minitask.exe]
No third-party Python dependencies; requires PowerShell for the documented example.
"""

import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile


repo = Path(__file__).resolve().parents[1]
binary = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else repo / "minitask.exe"
shell = shutil.which("pwsh") or shutil.which("powershell")
assert shell, "PowerShell is required to execute the documented example"


def ps_quote(value):
    return "'" + str(value).replace("'", "''") + "'"


with tempfile.TemporaryDirectory(prefix="skill-cli-", dir=repo / ".cache") as tmp:
    home = Path(tmp) / "保存先 with spaces"
    (home / "開発").mkdir(parents=True)
    (home / "別案件").mkdir()

    def run(*args, fails=False):
        result = subprocess.run(
            [str(binary), "--home", str(home), *args],
            capture_output=True, text=True, encoding="utf-8", check=False,
        )
        if fails:
            assert result.returncode == 1 and not result.stdout and result.stderr, result
            return None
        assert result.returncode == 0, result.stderr
        data = json.loads(result.stdout)
        assert data["schema_version"] == 2
        return data

    assert len(run("workspaces")["items"]) == 2
    note = run("add", "--workspace", "開発", "--kind", "note", "--title", "検証メモ",
               "--body", "自由な本文\n- [ ] メモ内のチェック項目\n記録済み") ["item"]
    assert "記録済み" in run("show", "--id", note["id"])["item"]["notes"]
    action = run("add", "--workspace", "開発", "--kind", "action", "--title", "記録の確認",
                 "--completion-condition", "検証メモに記録済みと書かれている。") ["item"]
    run("add", "--workspace", "別案件", "--kind", "action", "--title", "記録の確認")
    direction = run("add", "--workspace", "開発", "--kind", "direction", "--title", "本文を保持") ["item"]
    question = run("add", "--workspace", "開発", "--kind", "question", "--title", "公開日は？") ["item"]
    milestone = run("add", "--workspace", "開発", "--kind", "expectation", "--title", "初回デモ",
                    "--date", "2026-09-30") ["item"]
    assert milestone["date"] == "2026-09-30" and not milestone["completion_condition"]
    assert run("expectations", "--workspace", "開発", "--from", "2026-09-01", "--to", "2026-09-30")["items"][0]["id"] == milestone["id"]
    before = {p.relative_to(home): p.read_bytes() for p in home.rglob("*") if p.is_file()}
    resume = run("resume", "--workspace", "開発")
    assert set(resume) >= {"actions", "directions", "questions", "notes", "expectations", "issues"}
    assert len(resume["actions"]) == 1
    for key in ("actions", "directions", "questions", "notes", "expectations"):
        assert all(not ({"notes", "completion_condition", "supplement"} & item.keys()) for item in resume[key])
    summaries = run("tasks", "--workspace", "開発", "--query", "検証メモ")
    assert summaries["items"][0]["id"] == action["id"]
    assert "completion_condition" not in summaries["items"][0]
    assert run("show", "--id", action["id"])["item"]["completion_condition"]
    assert {p.relative_to(home): p.read_bytes() for p in home.rglob("*") if p.is_file()} == before

    # Independent Actions can stay in progress together; completing one leaves the other alone.
    parallel = run("add", "--workspace", "開発", "--kind", "action", "--title", "別の確認",
                   "--completion-condition", "別の検証が完了している") ["item"]
    for item in (action, parallel):
        run("set-status", "--id", item["id"], "--status", "in_progress",
            "--expected-hash", item["hash"])
    active_ids = {action["id"], parallel["id"]}
    assert {item["id"] for item in run("tasks", "--workspace", "開発", "--status", "in_progress")["items"]} == active_ids
    assert {item["id"] for item in run("resume", "--workspace", "開発")["actions"]} == active_ids
    parallel_before = run("show", "--id", parallel["id"])["item"]

    # Complete using the exact PowerShell block shipped in the skill.
    skill = (repo / "skills/using-minitask/SKILL.md").read_text(encoding="utf-8")
    examples = re.findall(r"```powershell\n(.*?)\n```", skill, re.S)
    entry = examples[0].replace("$bin = 'minitask'", "$bin = " + ps_quote(binary)).replace("'D:/Tasks'", ps_quote(home))
    result = subprocess.run([shell, "-NoProfile", "-NonInteractive", "-Command", entry],
                            capture_output=True, text=True, encoding="utf-8", check=False)
    assert result.returncode == 0, result.stderr
    example = examples[-1]
    setup = f"$bin={ps_quote(binary)}; $common=@('--home',{ps_quote(home)}); $id={ps_quote(action['id'])}; $evidence='showで検証メモの記録済みという本文を確認した';\n"
    result = subprocess.run([shell, "-NoProfile", "-NonInteractive", "-Command", setup + example],
                            capture_output=True, text=True, encoding="utf-8", check=False)
    assert result.returncode == 0, result.stderr
    completed = run("show", "--id", action["id"])["item"]
    assert completed["status"] == "completed" and "## Verification" in completed["notes"]
    assert run("show", "--id", parallel["id"])["item"] == parallel_before
    assert {item["id"] for item in run("resume", "--workspace", "開発")["actions"]} == {parallel["id"]}
    # A missing ID must terminate the example rather than parse null and continue.
    result = subprocess.run([shell, "-NoProfile", "-NonInteractive", "-Command",
                             setup + "$id='missing';\n" + example], capture_output=True, check=False)
    assert result.returncode != 0

    stale = milestone["hash"]
    path = Path(milestone["path"])
    path.write_bytes(path.read_bytes() + "\n人間の追記\n".encode())
    run("set-date", "--id", milestone["id"], "--field", "expectation", "--date", "2026-10-07",
        "--expected-hash", stale, fails=True)
    current = run("show", "--id", milestone["id"])["item"]
    assert current["date"] == "2026-09-30" and "人間の追記" in current["notes"]
    run("set-date", "--id", current["id"], "--field", "expectation", "--date", "none",
        "--expected-hash", current["hash"], fails=True)
    run("set-status", "--id", question["id"], "--status", "answered",
        "--expected-hash", question["hash"], fails=True)
    answered = run("set-status", "--id", question["id"], "--status", "answered",
                   "--expected-hash", question["hash"], "--evidence", "公開日は10月7日") ["item"]
    assert "## Answer" in answered["notes"] and answered["status"] == "answered"
    assert run("show", "--id", direction["id"])["item"]["status"] == "active"

print("PASS: documented PowerShell example, error stop, all entity types, scoped read-only queries, multiple in-progress Actions, stale hash, evidence and dates")
