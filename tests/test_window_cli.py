"""Pins for TS-32, the window option on the command line."""

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]

REASON = "the window closed before this item was started"

BACKLOG = {
    "tasks": [
        {"id": "task-1", "title": "alpha", "kind": "work", "question": ""},
        {"id": "task-2", "title": "gate", "kind": "decision", "question": "Ship the synthetic gate"},
        {"id": "task-3", "title": "omega", "kind": "work", "question": ""},
    ]
}

RESOLUTIONS = {
    "resolutions": [
        {"decision": "decision-task-2", "verdict": "approve", "note": "ship it tonight"},
    ]
}


def run_cli(*argv, cwd=ROOT):
    env = dict(os.environ)
    env["PYTHONPATH"] = str(ROOT) + os.pathsep + env.get("PYTHONPATH", "")
    return subprocess.run(
        [sys.executable, "-m", "thirdshift", *argv],
        cwd=cwd, capture_output=True, text=True, timeout=60, env=env,
    )


def write_json(path, data):
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def test_cli_run_with_a_window_records_the_window_block(tmp_path):
    backlog = tmp_path / "backlog.json"
    write_json(backlog, BACKLOG)
    result = run_cli("run", "--backlog", str(backlog), "--window-seconds", "2", cwd=tmp_path)
    assert result.returncode == 0, result.stderr
    record = json.loads(result.stdout)
    assert record["window"] == {
        "window_seconds": 2.0,
        "elapsed_seconds": 2.0,
        "started": 2,
        "unstarted": 1,
    }
    tail = [entry for entry in record["ran"] if entry["id"] == "task-3"]
    assert tail[0]["status"] == "unfinished"
    assert tail[0]["reason"] == REASON


def test_cli_run_without_a_window_has_no_window_block():
    result = run_cli("run", "--backlog", "examples/backlog.json")
    assert result.returncode == 0, result.stderr
    assert "window" not in json.loads(result.stdout)


def test_cli_brief_with_a_window_matches_the_committed_windowed_example(tmp_path):
    md_path = tmp_path / "windowed-brief.md"
    json_path = tmp_path / "windowed-brief.json"
    result = run_cli(
        "brief", "--backlog", "examples/backlog.json",
        "--window-seconds", "4",
        "--md", str(md_path), "--json", str(json_path),
    )
    assert result.returncode == 0, result.stderr
    assert md_path.read_text(encoding="utf-8") == (
        ROOT / "examples" / "windowed-brief.md"
    ).read_text(encoding="utf-8")
    assert json_path.read_text(encoding="utf-8") == (
        ROOT / "examples" / "windowed-brief.json"
    ).read_text(encoding="utf-8")


@pytest.mark.parametrize("step", ["run", "brief"])
@pytest.mark.parametrize("bad", ["-1", "soon"])
def test_cli_refuses_a_negative_or_non_numeric_window_with_one_error_line(step, bad, tmp_path):
    backlog = tmp_path / "backlog.json"
    write_json(backlog, BACKLOG)
    result = run_cli(step, "--backlog", str(backlog), "--window-seconds", bad, cwd=tmp_path)
    assert result.returncode == 2
    assert result.stdout == ""
    assert len(result.stderr.strip().splitlines()) == 1
    assert result.stderr.strip() != ""
    assert "Traceback" not in result.stderr


def test_cli_resolve_over_the_committed_windowed_examples_matches_the_committed_record():
    result = run_cli(
        "resolve",
        "--brief", "examples/windowed-brief.json",
        "--resolutions", "examples/resolutions.json",
    )
    assert result.returncode == 0, result.stderr
    expected = (ROOT / "examples" / "windowed-resolved.json").read_text(encoding="utf-8")
    assert result.stdout == expected
    out = json.loads(result.stdout)
    assert out["window"] == {
        "window_seconds": 4.0,
        "elapsed_seconds": 4.0,
        "started": 4,
        "unstarted": 1,
    }
    assert out["decision_counts"] == {"carried": 1, "queued": 0, "resolved": 1}


def test_cli_resolve_applies_to_a_windowed_brief_built_by_the_cli(tmp_path):
    backlog = tmp_path / "backlog.json"
    write_json(backlog, BACKLOG)
    brief_path = tmp_path / "brief.json"
    brief = run_cli(
        "brief", "--backlog", str(backlog),
        "--window-seconds", "2", "--json", str(brief_path), cwd=tmp_path,
    )
    assert brief.returncode == 0, brief.stderr
    resolutions = tmp_path / "resolutions.json"
    write_json(resolutions, RESOLUTIONS)
    result = run_cli(
        "resolve", "--brief", str(brief_path), "--resolutions", str(resolutions), cwd=tmp_path
    )
    assert result.returncode == 0, result.stderr
    out = json.loads(result.stdout)
    assert out["resolved"][0]["verdict"] == "approve"
    assert out["window"]["started"] == 2
    assert out["window"]["unstarted"] == 1
