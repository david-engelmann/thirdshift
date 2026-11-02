"""Pins for TS-8 and TS-9, the brief in two forms with the same content."""

import json

from thirdshift import FakeClock, Item, brief_json, brief_markdown, run_backlog

BACKLOG = [
    Item(item_id="task-1", title="alpha"),
    Item(item_id="task-2", title="broken", kind="fail"),
    Item(item_id="task-3", title="gate", kind="decision", question="Ship the synthetic gate"),
]


def make_brief():
    return run_backlog(BACKLOG, clock=FakeClock())


def test_json_brief_lists_ran_produced_and_decisions():
    data = json.loads(brief_json(make_brief()))
    assert [entry["id"] for entry in data["ran"]] == ["task-1", "task-2", "task-3"]
    assert data["produced"] == ["completed task-1"]
    assert data["decisions"][0]["question"] == "Ship the synthetic gate"
    assert data["decisions"][0]["options"] == ["approve", "reject"]


def test_markdown_brief_carries_the_same_content_as_the_json():
    brief = make_brief()
    md = brief_markdown(brief)
    data = json.loads(brief_json(brief))
    for entry in data["ran"]:
        assert entry["id"] in md
        assert entry["worker"] in md
        assert entry["status"] in md
    for output in data["produced"]:
        assert output in md
    for decision in data["decisions"]:
        assert decision["id"] in md
        assert decision["question"] in md
    assert "approve" in md
    assert "reject" in md


def test_markdown_brief_shows_the_reason_for_unfinished_work():
    md = brief_markdown(make_brief())
    assert "stub worker could not complete this task" in md


def test_two_runs_over_the_same_backlog_give_byte_identical_briefs():
    first = run_backlog(BACKLOG, clock=FakeClock())
    second = run_backlog(BACKLOG, clock=FakeClock())
    assert brief_json(first) == brief_json(second)
    assert brief_markdown(first) == brief_markdown(second)


def test_brief_counts_match_the_record():
    data = json.loads(brief_json(make_brief()))
    assert data["counts"] == {"backlog": 3, "finished": 2, "unfinished": 1, "recorded": 3}


def markdown_stock_block(md):
    section = md.split("## Stock", 1)[1]
    values = {}
    for line in section.splitlines():
        if line.startswith("- "):
            name, _, value = line[2:].rpartition(" ")
            values[name] = value
    return values


def test_the_two_brief_forms_carry_the_same_content():
    brief = make_brief()
    data = json.loads(brief_json(brief))
    md = brief_markdown(brief)
    for entry in data["ran"]:
        for key in ("id", "title", "worker", "status"):
            assert entry[key] in md
        if entry["output"]:
            assert entry["output"] in md
        if entry["reason"]:
            assert entry["reason"] in md
    for output in data["produced"]:
        assert output in md
    for decision in data["decisions"]:
        assert decision["id"] in md
        assert decision["item"] in md
        assert decision["question"] in md
    labels = {
        "workers": "workers",
        "stocked_at_start": "stocked at start",
        "stockouts_recorded": "stockouts recorded",
    }
    md_stock = markdown_stock_block(md)
    assert md_stock == {labels[key]: str(value) for key, value in data["stock"].items()}


def test_brief_stock_block_reports_the_run_not_a_standing_stockout():
    data = json.loads(brief_json(make_brief()))
    assert data["stock"] == {"workers": 2, "stocked_at_start": 3, "stockouts_recorded": 0}
    assert "stockout" not in data["stock"]


FOUR_TASKS = [
    Item(item_id="task-1", title="alpha"),
    Item(item_id="task-2", title="beta"),
    Item(item_id="task-3", title="gate", kind="decision", question="Ship the synthetic gate"),
    Item(item_id="task-4", title="omega"),
]


def test_briefs_do_not_depend_on_backlog_entry_order():
    forward = run_backlog(FOUR_TASKS, clock=FakeClock())
    backward = run_backlog(list(reversed(FOUR_TASKS)), clock=FakeClock())
    shuffled = run_backlog(
        [FOUR_TASKS[2], FOUR_TASKS[0], FOUR_TASKS[3], FOUR_TASKS[1]], clock=FakeClock()
    )
    assert backward == forward
    assert shuffled == forward
    assert brief_json(backward) == brief_json(forward)
    assert brief_json(shuffled) == brief_json(forward)
    assert brief_markdown(backward) == brief_markdown(forward)
    assert brief_markdown(shuffled) == brief_markdown(forward)
