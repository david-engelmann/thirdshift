"""A runner that works a backlog of synthetic tasks through the queue."""

import json
import math
from pathlib import Path

from .clock import FakeClock
from .queue import Item, WorkQueue

LEASE_SECONDS = 30.0

WINDOW_CLOSED_REASON = "the window closed before this item was started"


def _validate_window(window):
    """Return the window as a float, or None when no window was given.

    A window must be a non-negative finite number of seconds. Anything
    else is refused before any item is touched, so a bad window can
    never start a partial run.
    """
    if window is None:
        return None
    if isinstance(window, bool) or isinstance(window, str):
        raise ValueError("window must be a non-negative number of seconds")
    try:
        value = float(window)
    except (TypeError, ValueError, OverflowError):
        raise ValueError("window must be a non-negative number of seconds") from None
    if not math.isfinite(value) or value < 0:
        raise ValueError("window must be a non-negative number of seconds")
    return value


class BacklogError(Exception):
    """A backlog file could not be read as a backlog at all."""


def stub_worker(item):
    """Stub worker behaviour, fixed by task kind so runs are reproducible.

    A work task finishes with a plain output. A fail task never finishes
    and says why. A decision task finishes by queueing a decision.
    """
    if item.kind == "fail":
        return {"status": "unfinished", "output": "", "reason": "stub worker could not complete this task"}
    if item.kind == "decision":
        return {"status": "finished", "output": "", "reason": ""}
    return {"status": "finished", "output": "completed " + item.item_id, "reason": ""}


def parse_backlog_entries(data):
    """Unwrap parsed backlog JSON into its raw task entries.

    One shared reader for every caller. An object must carry a tasks
    list and a bare list is taken as the tasks themselves. Anything
    else is a load error, never a silent empty backlog.
    """
    if isinstance(data, dict):
        tasks = data.get("tasks")
        if not isinstance(tasks, list):
            raise BacklogError("backlog object carries no tasks list")
        return list(tasks)
    if isinstance(data, list):
        return list(data)
    raise BacklogError("backlog is neither a task list nor an object with a tasks list")


def read_backlog_file(path):
    target = Path(path)
    try:
        text = target.read_text(encoding="utf-8")
    except OSError as exc:
        raise BacklogError("backlog file cannot be read, " + str(path)) from exc
    except UnicodeDecodeError as exc:
        raise BacklogError("backlog file is not valid UTF-8, " + str(path)) from exc
    try:
        data = json.loads(text)
    except json.JSONDecodeError as exc:
        raise BacklogError("backlog file is not valid JSON, " + str(path)) from exc
    return parse_backlog_entries(data)


def load_backlog(path):
    """Read a backlog file and return its raw task entries.

    Entries come back unvalidated on purpose. The runner records the
    entries that cannot become tasks in the run record instead of
    failing the whole run over one bad entry.
    """
    return read_backlog_file(path)


def _recorded_entry(entry_id, title, reason):
    return {
        "id": entry_id,
        "title": title,
        "worker": "",
        "status": "unfinished",
        "output": "",
        "reason": reason,
    }


def _field_type_error(entry):
    """Name the first wrongly typed field on a backlog entry, or None.

    TS-16 covers entries whose fields carry the wrong types, not only
    entries that are missing or not objects. An id must be a non-empty
    string, and title, kind, and question must be strings when present.
    """
    item_id = entry.get("id")
    if not isinstance(item_id, str) or not item_id:
        return "malformed backlog entry, its id is not a non-empty string"
    for field in ("title", "kind", "question"):
        value = entry.get(field)
        if value is not None and not isinstance(value, str):
            return "malformed backlog entry, its " + field + " is not a string"
    return None

def _prepare_entry(entry, seen_ids):
    """Sort one backlog entry into a runnable item or a recorded entry.

    Returns (item, None) for an entry the runner will work, or
    (None, record entry) for an entry that is accounted for without
    running. Nothing is dropped either way.
    """
    if isinstance(entry, Item):
        item = entry
    elif isinstance(entry, dict):
        if "id" not in entry:
            title = entry.get("title", "")
            return None, _recorded_entry(
                "malformed-entry",
                title if isinstance(title, str) else "",
                "malformed backlog entry, it carries no id",
            )
        field_error = _field_type_error(entry)
        if field_error is not None:
            raw_title = entry.get("title", "")
            return None, _recorded_entry(
                "malformed-entry",
                raw_title if isinstance(raw_title, str) else "",
                field_error,
            )
        item = Item.from_dict(entry)
    else:
        return None, _recorded_entry(
            "malformed-entry",
            "",
            "malformed backlog entry, it is not a task object",
        )
    if item.question and item.kind != "decision":
        return None, _recorded_entry(
            item.item_id,
            item.title,
            "malformed decision task, it carries a question but its kind is not decision",
        )
    if item.item_id in seen_ids:
        return None, _recorded_entry(
            item.item_id,
            item.title,
            "duplicate item id " + item.item_id + ", only the first entry with this id runs",
        )
    seen_ids.add(item.item_id)
    return item, None


def run_backlog(entries, worker_ids=("worker-1", "worker-2"), clock=None, queue=None, advance_per_item=1.0, window=None):
    """Work every entry once, claiming through the queue, and return the record.

    The record lists one entry per backlog entry, ordered by item
    identifier, so nothing the runner dropped can hide. Workers are
    assigned over the identifier sorted work list, so the record never
    depends on the order entries appear in the backlog.

    When window is given, it is a number of seconds on the run clock
    measured from the start of the run. The runner reads the clock
    before it starts each item, and once the elapsed time has reached
    the window it starts no further item. Items left unstarted are
    recorded with the window closed reason, never claimed, and never
    counted as stockouts or completions. An item already started is
    worked to its outcome even when the clock passes the window while
    it is worked.
    """
    window_seconds = _validate_window(window)
    entries = list(entries)
    if entries and not worker_ids:
        raise ValueError("run_backlog needs at least one worker to work a non-empty backlog")
    if queue is None:
        if clock is None:
            clock = FakeClock()
        queue = WorkQueue(clock=clock)
    elif clock is None:
        clock = queue.clock
    for worker_id in worker_ids:
        queue.register_worker(worker_id)
    started_at = clock.now()

    seen_ids = set()
    runnable = []
    ran = []
    for entry in entries:
        item, problem = _prepare_entry(entry, seen_ids)
        if problem is not None:
            ran.append(problem)
        else:
            runnable.append(item)
    runnable.sort(key=lambda item: item.item_id)
    for item in runnable:
        queue.enqueue(item)
    stocked_at_start = len(queue.pending_ids())

    decisions = []
    started = 0
    unstarted = 0
    for index, item in enumerate(runnable):
        if window_seconds is not None and clock.now() - started_at >= window_seconds:
            unstarted += 1
            ran.append(_recorded_entry(item.item_id, item.title, WINDOW_CLOSED_REASON))
            continue
        started += 1
        worker_id = worker_ids[index % len(worker_ids)]
        claim = queue.claim(item.item_id, worker_id, lease_seconds=LEASE_SECONDS)
        result = stub_worker(item)
        queue.complete(item.item_id, claim.worker_id, result["status"])
        ran.append(
            {
                "id": item.item_id,
                "title": item.title,
                "worker": worker_id,
                "status": result["status"],
                "output": result["output"],
                "reason": result["reason"],
            }
        )
        if item.kind == "decision" and result["status"] == "finished":
            decisions.append(
                {
                    "id": "decision-" + item.item_id,
                    "item": item.item_id,
                    "question": item.question or item.title,
                    "options": ["approve", "reject"],
                }
            )
        clock.advance(advance_per_item)

    ran.sort(key=lambda entry: (entry["id"], json.dumps(entry, sort_keys=True)))
    record = {
        "ran": ran,
        "produced": [entry["output"] for entry in ran if entry["output"]],
        "decisions": sorted(decisions, key=lambda d: d["id"]),
        "counts": {
            "backlog": len(entries),
            "finished": sum(1 for e in ran if e["status"] == "finished"),
            "unfinished": sum(1 for e in ran if e["status"] != "finished"),
            "recorded": len(ran),
        },
        "stock": {
            "workers": len(worker_ids),
            "stocked_at_start": stocked_at_start,
            "stockouts_recorded": queue.stockouts,
        },
    }
    if window_seconds is not None:
        record["window"] = {
            "window_seconds": window_seconds,
            "elapsed_seconds": clock.now() - started_at,
            "started": started,
            "unstarted": unstarted,
        }
    return record
