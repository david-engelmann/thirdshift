"""Command line entry point, python -m thirdshift.

Subcommands
  enqueue  append one task to a backlog file
  run      work a backlog file and write the run record as JSON
  brief    work a backlog file and write the morning brief as Markdown and JSON
  resolve  fold a resolutions file into a brief JSON file and print the resolved record
"""

import argparse
import json
import sys
from pathlib import Path

from .brief import brief_json, brief_markdown
from .resolve import ResolutionError, apply_resolutions, load_resolutions, read_brief_file
from .runner import BacklogError, load_backlog, read_backlog_file, run_backlog


def _write_tasks(path, tasks):
    Path(path).write_text(json.dumps({"tasks": tasks}, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def cmd_enqueue(args):
    target = Path(args.file)
    tasks = read_backlog_file(target) if target.exists() else []
    if any(isinstance(task, dict) and task.get("id") == args.id for task in tasks):
        print("duplicate id " + args.id, file=sys.stderr)
        return 2
    task = {"id": args.id, "title": args.title, "kind": args.kind, "question": args.question}
    tasks.append(task)
    _write_tasks(args.file, tasks)
    print("enqueued " + args.id)
    return 0


def _window_value(raw):
    """Turn the window option text into a number of seconds, or None.

    A bad window follows the usual error discipline of the tool, so it
    raises BacklogError and the caller prints one error line and exits
    with status 2 rather than showing an argument parsing traceback.
    """
    if raw is None:
        return None
    try:
        value = float(raw)
    except (TypeError, ValueError) as exc:
        raise BacklogError("window seconds is not a number, " + str(raw)) from exc
    if value != value or value in (float("inf"), float("-inf")) or value < 0:
        raise BacklogError("window seconds must be a non-negative number, " + str(raw))
    return value


def cmd_run(args):
    record = run_backlog(load_backlog(args.backlog), window=_window_value(args.window_seconds))
    text = json.dumps(record, indent=2, sort_keys=True) + "\n"
    if args.result:
        Path(args.result).write_text(text, encoding="utf-8")
    print(text, end="")
    return 0


def cmd_brief(args):
    record = run_backlog(load_backlog(args.backlog), window=_window_value(args.window_seconds))
    if args.md:
        Path(args.md).write_text(brief_markdown(record), encoding="utf-8")
    if args.json:
        Path(args.json).write_text(brief_json(record), encoding="utf-8")
    if not args.md and not args.json:
        print(brief_markdown(record), end="")
    return 0


def cmd_resolve(args):
    record = read_brief_file(args.brief)
    resolved = apply_resolutions(record, load_resolutions(args.resolutions))
    print(json.dumps(resolved, indent=2, sort_keys=True) + "\n", end="")
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(prog="thirdshift")
    sub = parser.add_subparsers(dest="command", required=True)

    p_enqueue = sub.add_parser("enqueue")
    p_enqueue.add_argument("--file", required=True)
    p_enqueue.add_argument("--id", required=True)
    p_enqueue.add_argument("--title", required=True)
    p_enqueue.add_argument("--kind", default="work")
    p_enqueue.add_argument("--question", default="")
    p_enqueue.set_defaults(func=cmd_enqueue)

    p_run = sub.add_parser("run")
    p_run.add_argument("--backlog", required=True)
    p_run.add_argument("--result")
    p_run.add_argument("--window-seconds")
    p_run.set_defaults(func=cmd_run)

    p_brief = sub.add_parser("brief")
    p_brief.add_argument("--backlog", required=True)
    p_brief.add_argument("--md")
    p_brief.add_argument("--json")
    p_brief.add_argument("--window-seconds")
    p_brief.set_defaults(func=cmd_brief)

    p_resolve = sub.add_parser("resolve")
    p_resolve.add_argument("--brief", required=True)
    p_resolve.add_argument("--resolutions", required=True)
    p_resolve.set_defaults(func=cmd_resolve)

    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except (BacklogError, ResolutionError) as exc:
        print("error - " + str(exc), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
