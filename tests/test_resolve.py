"""Pins for TS-23 to TS-26, decision resolution over a brief record."""

import copy
import json

import pytest

from thirdshift import (
    FakeClock,
    Item,
    ResolutionError,
    apply_resolutions,
    load_resolutions,
    run_backlog,
)

BACKLOG = [
    Item(item_id="task-1", title="alpha"),
    Item(item_id="task-2", title="first gate", kind="decision", question="Ship the synthetic gate"),
    Item(item_id="task-3", title="second gate", kind="decision", question="Ship the second synthetic gate"),
]

APPROVE_FIRST = {"decision": "decision-task-2", "verdict": "approve", "note": "ship it tonight"}
REJECT_SECOND = {"decision": "decision-task-3", "verdict": "reject", "note": "hold for the next window"}


def make_record():
    return run_backlog(BACKLOG, clock=FakeClock())


def test_a_resolution_is_applied_and_visible():
    out = apply_resolutions(make_record(), [APPROVE_FIRST])
    assert len(out["resolved"]) == 1
    entry = out["resolved"][0]
    assert entry["id"] == "decision-task-2"
    assert entry["item"] == "task-2"
    assert entry["question"] == "Ship the synthetic gate"
    assert entry["verdict"] == "approve"
    assert entry["note"] == "ship it tonight"
    assert [d["id"] for d in out["decisions"]] == ["decision-task-3"]


def test_an_unnamed_decision_stays_queued_exactly_as_the_brief_carried_it():
    record = make_record()
    out = apply_resolutions(record, [APPROVE_FIRST])
    carried = {d["id"]: d for d in record["decisions"]}
    assert out["decisions"] == [carried["decision-task-3"]]


def test_decision_counts_add_up_for_partial_full_and_empty_resolution():
    record = make_record()
    partial = apply_resolutions(record, [APPROVE_FIRST])
    assert partial["decision_counts"] == {"carried": 2, "queued": 1, "resolved": 1}
    full = apply_resolutions(record, [APPROVE_FIRST, REJECT_SECOND])
    assert full["decision_counts"] == {"carried": 2, "queued": 0, "resolved": 2}
    assert full["decisions"] == []
    empty = apply_resolutions(record, [])
    assert empty["decision_counts"] == {"carried": 2, "queued": 2, "resolved": 0}
    assert empty["resolved"] == []
    for out in (partial, full, empty):
        counts = out["decision_counts"]
        assert counts["queued"] + counts["resolved"] == counts["carried"]


def test_the_resolved_list_follows_the_brief_order_not_the_file_order():
    out = apply_resolutions(make_record(), [REJECT_SECOND, APPROVE_FIRST])
    assert [d["id"] for d in out["resolved"]] == ["decision-task-2", "decision-task-3"]


def test_a_resolution_naming_a_decision_the_brief_does_not_carry_is_refused():
    record = make_record()
    snapshot = copy.deepcopy(record)
    stranger = {"decision": "decision-task-9", "verdict": "approve", "note": "who is this"}
    with pytest.raises(ResolutionError) as excinfo:
        apply_resolutions(record, [stranger])
    assert "decision-task-9" in str(excinfo.value)
    assert record == snapshot


def test_a_decision_named_twice_in_one_resolutions_list_is_refused():
    record = make_record()
    snapshot = copy.deepcopy(record)
    with pytest.raises(ResolutionError) as excinfo:
        apply_resolutions(record, [APPROVE_FIRST, dict(APPROVE_FIRST)])
    assert "decision-task-2" in str(excinfo.value)
    assert record == snapshot


def test_a_verdict_that_is_neither_approve_nor_reject_is_refused():
    record = make_record()
    snapshot = copy.deepcopy(record)
    shrug = {"decision": "decision-task-2", "verdict": "maybe", "note": "undecided"}
    with pytest.raises(ResolutionError) as excinfo:
        apply_resolutions(record, [shrug])
    assert "decision-task-2" in str(excinfo.value)
    assert record == snapshot


def test_a_refusal_after_a_valid_resolution_still_applies_nothing():
    record = make_record()
    snapshot = copy.deepcopy(record)
    stranger = {"decision": "decision-task-9", "verdict": "approve", "note": "who is this"}
    with pytest.raises(ResolutionError):
        apply_resolutions(record, [APPROVE_FIRST, stranger])
    assert record == snapshot


def test_a_resolution_that_is_not_an_object_is_refused():
    with pytest.raises(ResolutionError):
        apply_resolutions(make_record(), ["approve"])


def test_a_resolution_missing_its_note_is_refused():
    with pytest.raises(ResolutionError):
        apply_resolutions(make_record(), [{"decision": "decision-task-2", "verdict": "approve"}])


def test_a_resolution_with_a_wrongly_typed_verdict_is_refused():
    bad = {"decision": "decision-task-2", "verdict": 1, "note": "typed wrong"}
    with pytest.raises(ResolutionError):
        apply_resolutions(make_record(), [bad])


def test_a_record_with_no_decisions_list_is_refused():
    with pytest.raises(ResolutionError):
        apply_resolutions({"ran": []}, [APPROVE_FIRST])


def test_the_run_record_is_byte_identical_after_resolution():
    record = make_record()
    snapshot = copy.deepcopy(record)
    out = apply_resolutions(record, [APPROVE_FIRST, REJECT_SECOND])
    run_keys = ("ran", "produced", "counts", "stock")
    before = json.dumps({key: snapshot[key] for key in run_keys}, sort_keys=True)
    after = json.dumps({key: out[key] for key in run_keys}, sort_keys=True)
    assert before == after
    assert record == snapshot


def write_resolutions(tmp_path, text):
    path = tmp_path / "resolutions.json"
    path.write_text(text, encoding="utf-8")
    return path


def test_a_valid_resolutions_file_loads_its_entries(tmp_path):
    path = write_resolutions(
        tmp_path,
        json.dumps({"resolutions": [APPROVE_FIRST, REJECT_SECOND]}),
    )
    assert load_resolutions(path) == [APPROVE_FIRST, REJECT_SECOND]


def test_a_resolutions_file_with_invalid_json_fails_at_load(tmp_path):
    path = write_resolutions(tmp_path, "{not json")
    with pytest.raises(ResolutionError):
        load_resolutions(path)


def test_a_resolutions_object_with_no_resolutions_list_fails_at_load(tmp_path):
    path = write_resolutions(tmp_path, json.dumps({"verdicts": []}))
    with pytest.raises(ResolutionError):
        load_resolutions(path)


def test_a_resolutions_file_that_is_a_bare_list_fails_at_load(tmp_path):
    path = write_resolutions(tmp_path, json.dumps([APPROVE_FIRST]))
    with pytest.raises(ResolutionError):
        load_resolutions(path)


def test_a_missing_resolutions_file_fails_at_load(tmp_path):
    with pytest.raises(ResolutionError):
        load_resolutions(tmp_path / "nope.json")
