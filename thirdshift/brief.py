"""The morning brief, one content in Markdown and in JSON.

Both renderers take the run record itself. The brief carries exactly
what the record carries, so the two forms cannot drift apart in what
they show.
"""

import json


def brief_json(record):
    return json.dumps(record, indent=2, sort_keys=True) + "\n"


def brief_markdown(record):
    lines = []
    lines.append("# Morning brief")
    lines.append("")
    lines.append("## What ran")
    lines.append("")
    for entry in record["ran"]:
        parts = ["- " + entry["id"]]
        if entry["title"]:
            parts.append(entry["title"])
        if entry["worker"]:
            parts.append("by " + entry["worker"])
        parts.append(entry["status"])
        if entry["output"]:
            parts.append(entry["output"])
        if entry["reason"]:
            parts.append("reason - " + entry["reason"])
        lines.append(" - ".join(parts))
    lines.append("")
    lines.append("## What it produced")
    lines.append("")
    if record["produced"]:
        for output in record["produced"]:
            lines.append("- " + output)
    else:
        lines.append("- nothing")
    lines.append("")
    lines.append("## Decisions queued")
    lines.append("")
    if record["decisions"]:
        for decision in record["decisions"]:
            lines.append(
                "- " + decision["id"] + " for item " + decision["item"]
                + " - " + decision["question"]
                + " - answer approve or reject"
            )
    else:
        lines.append("- none")
    lines.append("")
    lines.append("## Counts")
    lines.append("")
    counts = record["counts"]
    lines.append("- backlog " + str(counts["backlog"]))
    lines.append("- finished " + str(counts["finished"]))
    lines.append("- unfinished " + str(counts["unfinished"]))
    lines.append("- recorded " + str(counts["recorded"]))
    lines.append("")
    lines.append("## Stock")
    lines.append("")
    stock = record["stock"]
    lines.append("- workers " + str(stock["workers"]))
    lines.append("- stocked at start " + str(stock["stocked_at_start"]))
    lines.append("- stockouts recorded " + str(stock["stockouts_recorded"]))
    lines.append("")
    return "\n".join(lines)
