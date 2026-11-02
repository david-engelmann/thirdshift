"""Decision resolution, the operator answers the decisions the brief queued."""

import copy
import json
from pathlib import Path

VERDICTS = ("approve", "reject")


class ResolutionError(Exception):
    """A resolutions file or brief record could not be read as recorded."""


def parse_resolutions_entries(data):
    """Unwrap parsed resolutions JSON into its raw resolution entries.

    One shared reader for every caller, parallel to the backlog reader.
    An object must carry a resolutions list. Anything else is a load
    error, never a silent empty resolutions list.
    """
    if isinstance(data, dict):
        resolutions = data.get("resolutions")
        if not isinstance(resolutions, list):
            raise ResolutionError("resolutions object carries no resolutions list")
        return list(resolutions)
    raise ResolutionError("resolutions file is not an object with a resolutions list")


def read_resolutions_file(path):
    target = Path(path)
    try:
        text = target.read_text(encoding="utf-8")
    except OSError as exc:
        raise ResolutionError("resolutions file cannot be read, " + str(path)) from exc
    except UnicodeDecodeError as exc:
        raise ResolutionError("resolutions file is not valid UTF-8, " + str(path)) from exc
    try:
        data = json.loads(text)
    except json.JSONDecodeError as exc:
        raise ResolutionError("resolutions file is not valid JSON, " + str(path)) from exc
    return parse_resolutions_entries(data)


def load_resolutions(path):
    """Read a resolutions file and return its raw resolution entries.

    Entries come back unvalidated on purpose, parallel to the backlog
    reader. apply_resolutions checks every entry before it folds any
    of them in, so a defective entry refuses the whole application.
    """
    return read_resolutions_file(path)


def read_brief_file(path):
    """Read a brief JSON file and return the record it carries.

    The file must hold a JSON object with a decisions list, the shape
    the brief step writes. Anything else is a load error.
    """
    target = Path(path)
    try:
        text = target.read_text(encoding="utf-8")
    except OSError as exc:
        raise ResolutionError("brief file cannot be read, " + str(path)) from exc
    except UnicodeDecodeError as exc:
        raise ResolutionError("brief file is not valid UTF-8, " + str(path)) from exc
    try:
        data = json.loads(text)
    except json.JSONDecodeError as exc:
        raise ResolutionError("brief file is not valid JSON, " + str(path)) from exc
    if not isinstance(data, dict) or not isinstance(data.get("decisions"), list):
        raise ResolutionError("brief file is not a brief record, it carries no decisions list")
    return data


def _check_resolution(entry, carried_ids, seen_ids):
    """Validate one resolution entry and return its decision, verdict, and note.

    Every defect is a refusal that names it, in the order the checks
    run. Structural defects come first, then the TS-25 refusals, so a
    resolutions list is either fully applicable or refused whole.
    """
    if not isinstance(entry, dict):
        raise ResolutionError("malformed resolution, it is not an object")
    decision = entry.get("decision")
    if not isinstance(decision, str) or not decision:
        raise ResolutionError("malformed resolution, its decision is not a non-empty string")
    verdict = entry.get("verdict")
    if not isinstance(verdict, str):
        raise ResolutionError(
            "malformed resolution for decision " + decision + ", its verdict is not a string"
        )
    note = entry.get("note")
    if not isinstance(note, str):
        raise ResolutionError(
            "malformed resolution for decision " + decision + ", its note is not a string"
        )
    if decision in seen_ids:
        raise ResolutionError("decision " + decision + " is named twice in the resolutions")
    if decision not in carried_ids:
        raise ResolutionError(
            "resolution names decision " + decision + ", which the brief does not carry"
        )
    if verdict not in VERDICTS:
        raise ResolutionError(
            "verdict for decision " + decision + " is neither approve nor reject"
        )
    seen_ids.add(decision)
    return decision, verdict, note


def _check_brief_decisions(decisions):
    """Validate the decisions a brief record carries and return their ids.

    Every entry must be an object carrying a non-empty string id and
    string item and question fields, the shape the brief step writes,
    and no id may repeat. A record outside that shape is refused whole,
    so no defect in the brief can escape the refusal discipline or fold
    one decision twice.
    """
    carried_ids = set()
    for entry in decisions:
        if not isinstance(entry, dict):
            raise ResolutionError("brief record carries a decision that is not an object")
        decision_id = entry.get("id")
        if not isinstance(decision_id, str) or not decision_id:
            raise ResolutionError(
                "brief record carries a decision whose id is not a non-empty string"
            )
        if not isinstance(entry.get("item"), str) or not isinstance(
            entry.get("question"), str
        ):
            raise ResolutionError(
                "brief record carries decision "
                + decision_id
                + " without a string item and question"
            )
        if decision_id in carried_ids:
            raise ResolutionError(
                "brief record carries decision " + decision_id + " twice"
            )
        carried_ids.add(decision_id)
    return carried_ids


def apply_resolutions(record, resolutions):
    """Fold recorded verdicts into a brief record and return the resolved record.

    Every resolution is validated before any is folded in, so a refusal
    produces no record and leaves the brief record exactly as it was.
    The run record is carried across untouched, and the outcome lands
    alongside it under the resolved list and the decision counts block.
    The returned record is a deep copy, so it shares no mutable state
    with the record it was resolved from.
    """
    decisions = record.get("decisions") if isinstance(record, dict) else None
    if not isinstance(decisions, list):
        raise ResolutionError("brief record carries no decisions list")
    carried_ids = _check_brief_decisions(decisions)
    decisions = copy.deepcopy(decisions)
    verdicts = {}
    seen_ids = set()
    for entry in resolutions:
        decision, verdict, note = _check_resolution(entry, carried_ids, seen_ids)
        verdicts[decision] = (verdict, note)

    queued = []
    resolved = []
    for entry in decisions:
        decision_id = entry.get("id") if isinstance(entry, dict) else None
        if decision_id in verdicts:
            verdict, note = verdicts[decision_id]
            folded = dict(entry)
            folded["verdict"] = verdict
            folded["note"] = note
            resolved.append(folded)
        else:
            queued.append(entry)

    result = copy.deepcopy(record)
    result["decisions"] = queued
    result["resolved"] = resolved
    result["decision_counts"] = {
        "carried": len(decisions),
        "queued": len(queued),
        "resolved": len(resolved),
    }
    return result
