"""Pins for TS-5, TS-6, and TS-7, the runner record and queued decisions."""

import pytest

from thirdshift import BacklogError, FakeClock, Item, WorkQueue, load_backlog, run_backlog

BACKLOG = [
    Item(item_id="task-1", title="alpha"),
    Item(item_id="task-2", title="broken", kind="fail"),
    Item(item_id="task-3", title="gate", kind="decision", question="Ship the synthetic gate"),
    Item(item_id="task-4", title="omega"),
]


def run():
    return run_backlog(BACKLOG, clock=FakeClock())


def test_every_backlog_item_appears_exactly_once_in_the_record():
    record = run()
    ids = [entry["id"] for entry in record["ran"]]
    assert sorted(ids) == ["task-1", "task-2", "task-3", "task-4"]
    assert len(ids) == len(set(ids))


def test_counts_add_up_to_the_backlog_size():
    record = run()
    counts = record["counts"]
    assert counts["backlog"] == 4
    assert counts["recorded"] == 4
    assert counts["finished"] + counts["unfinished"] == counts["backlog"]


def test_unfinished_items_carry_a_reason_and_finished_items_carry_output():
    record = run()
    by_id = {entry["id"]: entry for entry in record["ran"]}
    assert by_id["task-2"]["status"] == "unfinished"
    assert by_id["task-2"]["reason"] != ""
    assert by_id["task-1"]["status"] == "finished"
    assert by_id["task-1"]["output"] == "completed task-1"
    assert "completed task-2" not in record["produced"]


def test_every_entry_names_a_registered_worker():
    record = run()
    workers = {entry["worker"] for entry in record["ran"]}
    assert workers <= {"worker-1", "worker-2"}
    assert all(entry["worker"] for entry in record["ran"])


def test_decision_task_queues_a_decision_with_both_responses():
    record = run()
    assert len(record["decisions"]) == 1
    decision = record["decisions"][0]
    assert decision["item"] == "task-3"
    assert decision["question"] == "Ship the synthetic gate"
    assert decision["options"] == ["approve", "reject"]


def test_work_tasks_queue_no_decisions():
    record = run_backlog([Item(item_id="task-9", title="plain")], clock=FakeClock())
    assert record["decisions"] == []
    assert record["produced"] == ["completed task-9"]


class ClaimRecordingQueue(WorkQueue):
    """A queue that remembers every claim, so a runner that fabricates
    worker names without claiming leaves an empty claim log behind."""

    def __init__(self, clock):
        super().__init__(clock=clock)
        self.claim_calls = []

    def claim(self, item_id, worker_id, lease_seconds=60.0):
        self.claim_calls.append((item_id, worker_id))
        return super().claim(item_id, worker_id, lease_seconds=lease_seconds)


def test_runner_claims_every_item_it_records_through_the_queue():
    clock = FakeClock()
    queue = ClaimRecordingQueue(clock)
    record = run_backlog(BACKLOG, clock=clock, queue=queue)
    claimed = dict(queue.claim_calls)
    assert sorted(claimed) == ["task-1", "task-2", "task-3", "task-4"]
    assert len(queue.claim_calls) == 4
    for entry in record["ran"]:
        assert claimed[entry["id"]] == entry["worker"]


def test_duplicate_item_ids_are_recorded_and_the_run_still_completes():
    entries = [
        {"id": "task-1", "title": "alpha"},
        {"id": "task-2", "title": "beta"},
        {"id": "task-1", "title": "alpha again"},
    ]
    record = run_backlog(entries, clock=FakeClock())
    assert record["counts"] == {"backlog": 3, "finished": 2, "unfinished": 1, "recorded": 3}
    duplicates = [
        entry for entry in record["ran"]
        if entry["id"] == "task-1" and entry["status"] == "unfinished"
    ]
    assert len(duplicates) == 1
    assert "duplicate" in duplicates[0]["reason"]
    assert "completed task-1" in record["produced"]


def test_duplicate_item_objects_are_recorded_the_same_way():
    entries = [Item(item_id="task-1", title="alpha"), Item(item_id="task-1", title="alpha again")]
    record = run_backlog(entries, clock=FakeClock())
    assert record["counts"] == {"backlog": 2, "finished": 1, "unfinished": 1, "recorded": 2}


def test_a_question_without_the_decision_kind_is_recorded_not_run_as_work():
    entries = [{"id": "task-1", "title": "Approve the gate", "question": "Ship the synthetic gate"}]
    record = run_backlog(entries, clock=FakeClock())
    assert record["decisions"] == []
    assert record["produced"] == []
    entry = record["ran"][0]
    assert entry["status"] == "unfinished"
    assert "malformed decision" in entry["reason"]


def test_a_work_kind_with_a_question_is_also_a_malformed_decision():
    entries = [{"id": "task-1", "title": "Approve the gate", "kind": "work", "question": "Ship it"}]
    record = run_backlog(entries, clock=FakeClock())
    assert record["ran"][0]["status"] == "unfinished"
    assert "malformed decision" in record["ran"][0]["reason"]


def test_a_decision_task_without_a_question_falls_back_to_its_title():
    entries = [{"id": "task-1", "title": "Approve the synthetic gate", "kind": "decision"}]
    record = run_backlog(entries, clock=FakeClock())
    assert len(record["decisions"]) == 1
    assert record["decisions"][0]["question"] == "Approve the synthetic gate"


def test_malformed_entries_are_recorded_and_the_rest_still_run():
    entries = [
        {"title": "no id on this one"},
        "not a task at all",
        None,
        {"id": "task-1", "title": "alpha"},
    ]
    record = run_backlog(entries, clock=FakeClock())
    assert record["counts"] == {"backlog": 4, "finished": 1, "unfinished": 3, "recorded": 4}
    bad = [entry for entry in record["ran"] if entry["status"] == "unfinished"]
    assert len(bad) == 3
    assert all(entry["reason"] for entry in bad)


def test_an_entry_with_only_an_id_still_runs():
    record = run_backlog([{"id": "task-7"}], clock=FakeClock())
    assert record["counts"]["finished"] == 1
    assert record["produced"] == ["completed task-7"]


def test_no_workers_for_a_non_empty_backlog_is_a_clear_error():
    with pytest.raises(ValueError):
        run_backlog([Item(item_id="task-1", title="alpha")], worker_ids=(), clock=FakeClock())
    empty = run_backlog([], worker_ids=(), clock=FakeClock())
    assert empty["counts"] == {"backlog": 0, "finished": 0, "unfinished": 0, "recorded": 0}


def test_load_backlog_rejects_invalid_json(tmp_path):
    path = tmp_path / "backlog.json"
    path.write_text("{not json")
    with pytest.raises(BacklogError):
        load_backlog(path)


def test_load_backlog_rejects_an_object_without_a_tasks_list(tmp_path):
    path = tmp_path / "backlog.json"
    path.write_text('{"items": []}')
    with pytest.raises(BacklogError):
        load_backlog(path)


def test_wrongly_typed_fields_are_recorded_not_crashing():
    record = run_backlog([
        {"id": 5, "title": "numeric id"},
        {"id": ["a"], "title": "list id"},
        {"id": "t-title", "title": 3},
        {"id": "t-kind", "kind": 7},
        {"id": "t-question", "kind": "decision", "question": 9},
        {"id": "", "title": "empty id"},
        {"id": "good", "title": "fine"},
    ])
    reasons = [entry.get("reason", "") for entry in record["ran"]]
    assert record["counts"]["backlog"] == 7
    assert record["counts"]["recorded"] == 7
    assert record["counts"]["finished"] == 1
    assert any("id is not a non-empty string" in r for r in reasons)
    assert any("title is not a string" in r for r in reasons)
    assert any("kind is not a string" in r for r in reasons)
    assert any("question is not a string" in r for r in reasons)
