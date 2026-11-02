"""Pins for TS-27, the resolve step on the command line."""

import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

BACKLOG = {
    "tasks": [
        {"id": "task-1", "title": "alpha", "kind": "work", "question": ""},
        {"id": "task-2", "title": "gate", "kind": "decision", "question": "Ship the synthetic gate"},
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


def make_brief_json(tmp_path):
    backlog = tmp_path / "backlog.json"
    write_json(backlog, BACKLOG)
    brief_path = tmp_path / "brief.json"
    result = run_cli("brief", "--backlog", str(backlog), "--json", str(brief_path), cwd=tmp_path)
    assert result.returncode == 0, result.stderr
    return brief_path


def make_resolutions(tmp_path, data=RESOLUTIONS):
    path = tmp_path / "resolutions.json"
    write_json(path, data)
    return path


def test_cli_resolve_prints_the_resolved_record(tmp_path):
    brief_path = make_brief_json(tmp_path)
    resolutions = make_resolutions(tmp_path)
    result = run_cli("resolve", "--brief", str(brief_path), "--resolutions", str(resolutions), cwd=tmp_path)
    assert result.returncode == 0, result.stderr
    out = json.loads(result.stdout)
    assert out["decision_counts"] == {"carried": 1, "queued": 0, "resolved": 1}
    assert out["decisions"] == []
    assert out["resolved"][0]["id"] == "decision-task-2"
    assert out["resolved"][0]["verdict"] == "approve"
    assert out["resolved"][0]["note"] == "ship it tonight"
    assert out["counts"] == {"backlog": 2, "finished": 2, "unfinished": 0, "recorded": 2}


def test_cli_resolve_refuses_an_unknown_decision_with_one_error_line(tmp_path):
    brief_path = make_brief_json(tmp_path)
    stranger = {
        "resolutions": [
            {"decision": "decision-task-9", "verdict": "approve", "note": "who is this"},
        ]
    }
    resolutions = make_resolutions(tmp_path, stranger)
    result = run_cli("resolve", "--brief", str(brief_path), "--resolutions", str(resolutions), cwd=tmp_path)
    assert result.returncode == 2
    assert result.stdout == ""
    assert len(result.stderr.strip().splitlines()) == 1
    assert "decision-task-9" in result.stderr
    assert "Traceback" not in result.stderr


def test_cli_resolve_refuses_a_verdict_that_is_neither_approve_nor_reject(tmp_path):
    brief_path = make_brief_json(tmp_path)
    shrug = {
        "resolutions": [
            {"decision": "decision-task-2", "verdict": "maybe", "note": "undecided"},
        ]
    }
    resolutions = make_resolutions(tmp_path, shrug)
    result = run_cli("resolve", "--brief", str(brief_path), "--resolutions", str(resolutions), cwd=tmp_path)
    assert result.returncode == 2
    assert result.stdout == ""
    assert "Traceback" not in result.stderr


def test_cli_resolve_refuses_a_malformed_resolutions_file(tmp_path):
    brief_path = make_brief_json(tmp_path)
    bad = tmp_path / "resolutions.json"
    bad.write_text("{not json", encoding="utf-8")
    result = run_cli("resolve", "--brief", str(brief_path), "--resolutions", str(bad), cwd=tmp_path)
    assert result.returncode == 2
    assert result.stdout == ""
    assert len(result.stderr.strip().splitlines()) == 1
    assert "Traceback" not in result.stderr


def test_cli_resolve_refuses_a_missing_brief_file(tmp_path):
    resolutions = make_resolutions(tmp_path)
    missing = tmp_path / "nope.json"
    result = run_cli("resolve", "--brief", str(missing), "--resolutions", str(resolutions), cwd=tmp_path)
    assert result.returncode == 2
    assert result.stdout == ""
    assert "Traceback" not in result.stderr


def test_cli_resolve_over_the_committed_examples_matches_the_committed_resolved_record():
    result = run_cli(
        "resolve",
        "--brief", "examples/brief.json",
        "--resolutions", "examples/resolutions.json",
    )
    assert result.returncode == 0, result.stderr
    expected = (ROOT / "examples" / "resolved.json").read_text(encoding="utf-8")
    assert result.stdout == expected


def test_cli_resolve_refuses_a_brief_with_a_non_string_decision_id(tmp_path):
    brief = tmp_path / "brief.json"
    write_json(brief, {"decisions": [
        {"id": "d1", "item": "t1", "question": "q1", "options": ["approve", "reject"]},
        {"id": ["x"], "item": "t2", "question": "q2", "options": ["approve", "reject"]},
    ], "ran": [], "produced": [], "counts": {}, "stock": {}})
    resolutions = tmp_path / "resolutions.json"
    write_json(resolutions, {"resolutions": [{"decision": "d1", "verdict": "approve", "note": "n"}]})
    result = run_cli("resolve", "--brief", str(brief), "--resolutions", str(resolutions), cwd=tmp_path)
    assert result.returncode == 2
    assert result.stdout == ""
    assert result.stderr.count("\n") == 1
    assert "Traceback" not in result.stderr


def test_cli_resolve_refuses_a_brief_file_with_no_decisions_list(tmp_path):
    not_a_brief = tmp_path / "brief.json"
    write_json(not_a_brief, {"tasks": []})
    resolutions = make_resolutions(tmp_path)
    result = run_cli("resolve", "--brief", str(not_a_brief), "--resolutions", str(resolutions), cwd=tmp_path)
    assert result.returncode == 2
    assert result.stdout == ""
    assert "Traceback" not in result.stderr
