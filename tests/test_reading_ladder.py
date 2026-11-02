"""Pins for TS-34, one reading ladder behind the three file surfaces.

Characterization pins, written against the readers as they stood
before the ladder was shared. The backlog file, the brief file, and
the resolutions file each refuse an unreadable path, a file that is
not valid UTF-8, and a file that is not valid JSON, on the command
line with exit code 2 and one error line and at the library surface
with the error type and message of their own surface. The three
message sets differ only in the noun each surface names, and these
pins state them as one set, so they must pass before the refactor
and after it alike.
"""

import os
import subprocess
import sys
from pathlib import Path

import pytest

from thirdshift.resolve import (
    ResolutionError,
    load_resolutions,
    read_brief_file,
    read_resolutions_file,
)
from thirdshift.runner import BacklogError, load_backlog, read_backlog_file

ROOT = Path(__file__).resolve().parents[1]

REFUSAL_KINDS = ("cannot be read, ", "is not valid UTF-8, ", "is not valid JSON, ")


def run_cli(*argv, cwd=ROOT):
    env = dict(os.environ)
    env["PYTHONPATH"] = str(ROOT) + os.pathsep + env.get("PYTHONPATH", "")
    return subprocess.run(
        [sys.executable, "-m", "thirdshift", *argv],
        cwd=cwd, capture_output=True, text=True, timeout=60, env=env,
    )


def bad_files(tmp_path, stem):
    """One unreadable, one non UTF-8, and one non JSON file, in kind order."""
    missing = tmp_path / (stem + "-missing.json")
    not_utf8 = tmp_path / (stem + "-not-utf8.json")
    not_utf8.write_bytes(b'{"tasks": [{"id": "x", "title": "\xff\xfe"}]}')
    not_json = tmp_path / (stem + "-not-json.json")
    not_json.write_text("{not json", encoding="utf-8")
    return [missing, not_utf8, not_json]


def expected_messages(noun, paths):
    return [
        noun + " " + kind + str(path) for kind, path in zip(REFUSAL_KINDS, paths)
    ]


def test_backlog_surface_refusals_on_the_command_line(tmp_path):
    paths = bad_files(tmp_path, "backlog")
    for path, message in zip(paths, expected_messages("backlog file", paths)):
        result = run_cli("run", "--backlog", str(path), cwd=tmp_path)
        assert result.returncode == 2
        assert result.stdout == ""
        assert result.stderr == "error - " + message + "\n"


def test_backlog_surface_refusals_at_the_library_surface(tmp_path):
    paths = bad_files(tmp_path, "backlog")
    for path, message in zip(paths, expected_messages("backlog file", paths)):
        for reader in (read_backlog_file, load_backlog):
            with pytest.raises(BacklogError) as caught:
                reader(path)
            assert str(caught.value) == message


def test_resolutions_surface_refusals_on_the_command_line(tmp_path):
    paths = bad_files(tmp_path, "resolutions")
    for path, message in zip(paths, expected_messages("resolutions file", paths)):
        result = run_cli(
            "resolve",
            "--brief", str(ROOT / "examples" / "brief.json"),
            "--resolutions", str(path),
            cwd=tmp_path,
        )
        assert result.returncode == 2
        assert result.stdout == ""
        assert result.stderr == "error - " + message + "\n"


def test_resolutions_surface_refusals_at_the_library_surface(tmp_path):
    paths = bad_files(tmp_path, "resolutions")
    for path, message in zip(paths, expected_messages("resolutions file", paths)):
        for reader in (read_resolutions_file, load_resolutions):
            with pytest.raises(ResolutionError) as caught:
                reader(path)
            assert str(caught.value) == message


def test_brief_surface_refusals_on_the_command_line(tmp_path):
    paths = bad_files(tmp_path, "brief")
    for path, message in zip(paths, expected_messages("brief file", paths)):
        result = run_cli(
            "resolve",
            "--brief", str(path),
            "--resolutions", str(ROOT / "examples" / "resolutions.json"),
            cwd=tmp_path,
        )
        assert result.returncode == 2
        assert result.stdout == ""
        assert result.stderr == "error - " + message + "\n"


def test_brief_surface_refusals_at_the_library_surface(tmp_path):
    paths = bad_files(tmp_path, "brief")
    for path, message in zip(paths, expected_messages("brief file", paths)):
        with pytest.raises(ResolutionError) as caught:
            read_brief_file(path)
        assert str(caught.value) == message


def test_run_over_the_example_backlog_matches_the_committed_example_brief_json():
    result = run_cli("run", "--backlog", "examples/backlog.json")
    assert result.returncode == 0, result.stderr
    expected = (ROOT / "examples" / "brief.json").read_text(encoding="utf-8")
    assert result.stdout == expected


def test_library_readers_return_the_committed_example_entries():
    entries = load_backlog(ROOT / "examples" / "backlog.json")
    assert [entry["id"] for entry in entries] == [
        "task-1", "task-2", "task-3", "task-4", "task-5",
    ]
    resolutions = load_resolutions(ROOT / "examples" / "resolutions.json")
    assert resolutions == [
        {
            "decision": "decision-task-4",
            "note": "Synthetic build approved for the overnight run",
            "verdict": "approve",
        }
    ]
    record = read_brief_file(ROOT / "examples" / "brief.json")
    assert [decision["id"] for decision in record["decisions"]] == ["decision-task-4"]
    assert record["counts"] == {
        "backlog": 5, "finished": 4, "recorded": 5, "unfinished": 1,
    }
