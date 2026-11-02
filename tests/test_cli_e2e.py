"""End to end, the CLI over the committed example backlog and over a built one."""

import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def run_cli(*argv, cwd=ROOT):
    env = dict(os.environ)
    env["PYTHONPATH"] = str(ROOT) + os.pathsep + env.get("PYTHONPATH", "")
    return subprocess.run(
        [sys.executable, "-m", "thirdshift", *argv],
        cwd=cwd, capture_output=True, text=True, timeout=60, env=env,
    )


def test_cli_run_over_example_backlog_records_all_five_tasks():
    result = run_cli("run", "--backlog", "examples/backlog.json")
    assert result.returncode == 0, result.stderr
    record = json.loads(result.stdout)
    assert record["counts"] == {"backlog": 5, "finished": 4, "unfinished": 1, "recorded": 5}
    assert len(record["decisions"]) == 1


def test_cli_brief_over_example_backlog_matches_the_committed_example_brief(tmp_path):
    md_path = tmp_path / "brief.md"
    json_path = tmp_path / "brief.json"
    result = run_cli("brief", "--backlog", "examples/backlog.json", "--md", str(md_path), "--json", str(json_path))
    assert result.returncode == 0, result.stderr
    assert md_path.read_text(encoding="utf-8") == (ROOT / "examples" / "brief.md").read_text(encoding="utf-8")
    assert json_path.read_text(encoding="utf-8") == (ROOT / "examples" / "brief.json").read_text(encoding="utf-8")


def test_cli_enqueue_run_brief_end_to_end(tmp_path):
    backlog = tmp_path / "backlog.json"
    first = run_cli("enqueue", "--file", str(backlog), "--id", "task-1", "--title", "alpha", cwd=tmp_path)
    assert first.returncode == 0, first.stderr
    second = run_cli(
        "enqueue", "--file", str(backlog), "--id", "task-2", "--title", "gate",
        "--kind", "decision", "--question", "Ship the synthetic gate", cwd=tmp_path,
    )
    assert second.returncode == 0, second.stderr
    md_path = tmp_path / "brief.md"
    json_path = tmp_path / "brief.json"
    brief = run_cli("brief", "--backlog", str(backlog), "--md", str(md_path), "--json", str(json_path), cwd=tmp_path)
    assert brief.returncode == 0, brief.stderr
    data = json.loads(json_path.read_text(encoding="utf-8"))
    assert data["counts"]["recorded"] == 2
    assert data["decisions"][0]["question"] == "Ship the synthetic gate"
    assert "task-1" in md_path.read_text(encoding="utf-8")


def test_cli_enqueue_rejects_a_duplicate_id(tmp_path):
    backlog = tmp_path / "backlog.json"
    assert run_cli("enqueue", "--file", str(backlog), "--id", "task-1", "--title", "alpha", cwd=tmp_path).returncode == 0
    again = run_cli("enqueue", "--file", str(backlog), "--id", "task-1", "--title", "alpha again", cwd=tmp_path)
    assert again.returncode == 2


def test_cli_rejects_invalid_json_with_a_clean_error(tmp_path):
    backlog = tmp_path / "backlog.json"
    backlog.write_text("{not json")
    result = run_cli("brief", "--backlog", str(backlog), cwd=tmp_path)
    assert result.returncode == 2
    assert result.stderr.strip() != ""
    assert "Traceback" not in result.stderr


def test_cli_rejects_a_backlog_object_without_a_tasks_list(tmp_path):
    backlog = tmp_path / "backlog.json"
    backlog.write_text(json.dumps({"items": []}))
    result = run_cli("run", "--backlog", str(backlog), cwd=tmp_path)
    assert result.returncode == 2
    assert result.stderr.strip() != ""
    assert "Traceback" not in result.stderr


def test_cli_invalid_utf8_backlog_exits_2_without_traceback(tmp_path):
    import subprocess, sys
    backlog = tmp_path / "bad.json"
    backlog.write_bytes(b'{"tasks": [{"id": "x", "title": "\xff\xfe"}]}')
    result = subprocess.run(
        [sys.executable, "-m", "thirdshift", "run", "--backlog", str(backlog)],
        capture_output=True, text=True, cwd=str(ROOT),
        env={"PYTHONPATH": str(ROOT), "PATH": "/usr/bin:/bin"},
    )
    assert result.returncode == 2
    assert "Traceback" not in result.stderr
