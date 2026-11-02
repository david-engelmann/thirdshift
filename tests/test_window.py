"""Pins for TS-29 to TS-31, the run window and the record it leaves."""

import copy
import json
from pathlib import Path

import pytest

from thirdshift import (
    FakeClock,
    Item,
    WorkQueue,
    apply_resolutions,
    brief_json,
    brief_markdown,
    load_backlog,
    run_backlog,
)

ROOT = Path(__file__).resolve().parents[1]

REASON = "the window closed before this item was started"

BACKLOG = [
    Item(item_id="task-1", title="alpha"),
    Item(item_id="task-2", title="beta"),
    Item(item_id="task-3", title="gamma"),
    Item(item_id="task-4", title="gate", kind="decision", question="Ship the synthetic gate"),
    Item(item_id="task-5", title="omega"),
]


def by_id(record):
    return {entry["id"]: entry for entry in record["ran"]}


def test_a_window_that_closes_mid_backlog_leaves_the_exact_tail_unstarted():
    record = run_backlog(BACKLOG, clock=FakeClock(), window=2)
    entries = by_id(record)
    assert entries["task-1"]["status"] == "finished"
    assert entries["task-2"]["status"] == "finished"
    for item_id in ("task-3", "task-4", "task-5"):
        assert entries[item_id]["status"] == "unfinished"
        assert entries[item_id]["reason"] == REASON
        assert entries[item_id]["worker"] == ""
        assert entries[item_id]["output"] == ""
        assert entries[item_id]["title"] != ""
    assert record["counts"] == {"backlog": 5, "finished": 2, "unfinished": 3, "recorded": 5}
    assert record["produced"] == ["completed task-1", "completed task-2"]


def test_the_window_reaching_its_end_exactly_starts_no_further_item():
    record = run_backlog(BACKLOG, clock=FakeClock(), window=1)
    entries = by_id(record)
    assert entries["task-1"]["status"] == "finished"
    assert entries["task-2"]["status"] == "unfinished"
    assert entries["task-2"]["reason"] == REASON
    assert record["window"]["started"] == 1


class ClaimRecordingQueue(WorkQueue):
    """A queue that remembers every claim, so an unstarted item that was
    claimed anyway leaves evidence behind."""

    def __init__(self, clock):
        super().__init__(clock=clock)
        self.claim_calls = []

    def claim(self, item_id, worker_id, lease_seconds=60.0):
        self.claim_calls.append((item_id, worker_id))
        return super().claim(item_id, worker_id, lease_seconds=lease_seconds)


def test_window_closed_items_carry_no_claim_and_no_lease_and_no_stockout():
    clock = FakeClock()
    queue = ClaimRecordingQueue(clock)
    record = run_backlog(BACKLOG, clock=clock, queue=queue, window=2)
    assert [item_id for item_id, _ in queue.claim_calls] == ["task-1", "task-2"]
    assert sorted(queue.done) == ["task-1", "task-2"]
    assert queue.claims == {}
    assert queue.stockouts == 0
    assert record["stock"] == {"workers": 2, "stocked_at_start": 5, "stockouts_recorded": 0}


def test_a_window_closed_decision_item_queues_no_decision():
    record = run_backlog(BACKLOG, clock=FakeClock(), window=2)
    assert record["decisions"] == []


def test_an_item_started_inside_the_window_is_worked_past_its_end():
    entries = [
        Item(item_id="task-1", title="alpha"),
        Item(item_id="task-2", title="beta"),
        Item(item_id="task-3", title="gamma"),
    ]
    record = run_backlog(entries, clock=FakeClock(), advance_per_item=5.0, window=3)
    items = by_id(record)
    assert items["task-1"]["status"] == "finished"
    assert items["task-1"]["output"] == "completed task-1"
    assert items["task-2"]["reason"] == REASON
    assert items["task-3"]["reason"] == REASON
    assert record["window"] == {
        "window_seconds": 3.0,
        "elapsed_seconds": 5.0,
        "started": 1,
        "unstarted": 2,
    }


def test_a_zero_window_starts_nothing_and_still_produces_a_brief():
    record = run_backlog(BACKLOG, clock=FakeClock(), window=0)
    assert record["counts"] == {"backlog": 5, "finished": 0, "unfinished": 5, "recorded": 5}
    assert record["produced"] == []
    assert record["decisions"] == []
    assert all(entry["reason"] == REASON for entry in record["ran"])
    assert record["window"] == {
        "window_seconds": 0.0,
        "elapsed_seconds": 0.0,
        "started": 0,
        "unstarted": 5,
    }
    data = json.loads(brief_json(record))
    assert data["window"]["started"] == 0
    md = brief_markdown(record)
    assert REASON in md
    assert "## Window" in md


def test_a_run_that_finishes_inside_the_window_records_no_window_closed_item():
    record = run_backlog(BACKLOG, clock=FakeClock(), window=100)
    assert all(entry["reason"] != REASON for entry in record["ran"])
    assert record["counts"]["finished"] == 5
    assert record["window"] == {
        "window_seconds": 100.0,
        "elapsed_seconds": 5.0,
        "started": 5,
        "unstarted": 0,
    }
    assert len(record["decisions"]) == 1


def markdown_window_block(md):
    section = md.split("## Window", 1)[1]
    values = {}
    for line in section.splitlines():
        if line.startswith("- "):
            name, _, value = line[2:].rpartition(" ")
            values[name] = value
    return values


def test_the_window_block_numbers_agree_in_both_brief_renderings():
    record = run_backlog(BACKLOG, clock=FakeClock(), window=2)
    data = json.loads(brief_json(record))
    assert data["window"] == {
        "window_seconds": 2.0,
        "elapsed_seconds": 2.0,
        "started": 2,
        "unstarted": 3,
    }
    md_values = markdown_window_block(brief_markdown(record))
    assert md_values == {
        "window seconds": str(data["window"]["window_seconds"]),
        "elapsed seconds": str(data["window"]["elapsed_seconds"]),
        "started": str(data["window"]["started"]),
        "unstarted": str(data["window"]["unstarted"]),
    }


def test_the_window_is_measured_from_the_run_start_on_the_injected_clock():
    record = run_backlog(BACKLOG, clock=FakeClock(start=100.0), window=2)
    assert record["window"] == {
        "window_seconds": 2.0,
        "elapsed_seconds": 2.0,
        "started": 2,
        "unstarted": 3,
    }
    assert by_id(record)["task-1"]["status"] == "finished"


def test_a_fractional_window_gates_on_the_fraction():
    record = run_backlog(BACKLOG, clock=FakeClock(), advance_per_item=0.5, window=1.2)
    assert record["window"]["started"] == 3
    assert record["window"]["unstarted"] == 2
    assert record["window"]["elapsed_seconds"] == 1.5


def test_no_window_leaves_the_record_and_the_brief_without_a_window_block():
    record = run_backlog(BACKLOG, clock=FakeClock())
    assert "window" not in record
    assert "window" not in json.loads(brief_json(record))
    assert "## Window" not in brief_markdown(record)


def test_no_window_output_stays_byte_identical_to_the_committed_example():
    entries = load_backlog(ROOT / "examples" / "backlog.json")
    record = run_backlog(entries, clock=FakeClock())
    assert brief_json(record) == (ROOT / "examples" / "brief.json").read_text(encoding="utf-8")
    assert brief_markdown(record) == (ROOT / "examples" / "brief.md").read_text(encoding="utf-8")


@pytest.mark.parametrize(
    "bad_window",
    [-1, -0.5, "2", "soon", True, float("nan"), float("inf")],
)
def test_a_window_that_is_negative_or_not_a_number_is_refused_before_any_work(bad_window):
    clock = FakeClock()
    queue = WorkQueue(clock=clock)
    with pytest.raises(ValueError):
        run_backlog(BACKLOG, clock=clock, queue=queue, window=bad_window)
    assert queue.workers == []
    assert queue.claims == {}
    assert queue.done == {}
    assert queue.stockouts == 0


def test_resolutions_apply_to_a_windowed_run_exactly_as_before():
    entries = [
        Item(item_id="task-1", title="alpha"),
        Item(item_id="task-2", title="gate", kind="decision", question="Ship the synthetic gate"),
        Item(item_id="task-3", title="omega"),
    ]
    record = run_backlog(entries, clock=FakeClock(), window=2)
    assert [d["id"] for d in record["decisions"]] == ["decision-task-2"]
    assert by_id(record)["task-3"]["reason"] == REASON
    snapshot = copy.deepcopy(record)
    out = apply_resolutions(
        record,
        [{"decision": "decision-task-2", "verdict": "approve", "note": "ship it tonight"}],
    )
    assert out["decisions"] == []
    assert out["resolved"][0]["id"] == "decision-task-2"
    assert out["resolved"][0]["verdict"] == "approve"
    assert out["resolved"][0]["note"] == "ship it tonight"
    assert out["decision_counts"] == {"carried": 1, "queued": 0, "resolved": 1}
    assert out["window"] == snapshot["window"]
    for key in ("ran", "produced", "counts", "stock"):
        assert out[key] == snapshot[key]
    assert record == snapshot


# The Round 3 review pins, TS-33.


def test_a_decimal_or_fraction_window_is_accepted_as_the_number_it_names():
    from decimal import Decimal
    from fractions import Fraction

    record = run_backlog(BACKLOG, clock=FakeClock(), window=Decimal("2"))
    assert record["window"]["window_seconds"] == 2.0
    assert record["window"]["started"] == 2
    record = run_backlog(BACKLOG, clock=FakeClock(), window=Fraction(3, 2))
    assert record["window"]["window_seconds"] == 1.5
    assert record["window"]["started"] == 2


def test_a_window_too_large_to_convert_is_a_validation_error():
    with pytest.raises(ValueError):
        run_backlog(BACKLOG, clock=FakeClock(), window=10**400)
