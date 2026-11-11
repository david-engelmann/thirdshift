# thirdshift

Runs a stocked backlog of agent tasks through a usage window, records what finished and what did not, hands the unfinished items to the next shift, and writes a morning brief of the run.

Thirdshift is a command line tool that works a backlog of agent tasks to a full record. The operator writes the backlog as a JSON file of tasks. The tool claims each task for a worker under a lease, works the tasks inside the run window the operator sets, and records every task as finished or unfinished with its reason. A run ends as a JSON record, as a morning brief in Markdown and JSON, as a resolved record once the operator answers the decisions the brief queued, or as a carried backlog holding exactly the unfinished tasks for the next shift.

[![CI](https://github.com/david-engelmann/thirdshift/actions/workflows/ci.yml/badge.svg)](https://github.com/david-engelmann/thirdshift/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

```console
$ ./target/debug/thirdshift brief --backlog examples/backlog.json
# Morning brief

## What ran

- task-1 - Sort the synthetic inbox - by worker-1 - finished - completed task-1
- task-2 - Draft the synthetic summary - by worker-2 - finished - completed task-2
- task-3 - Rebuild the synthetic index - by worker-1 - unfinished - reason - stub worker could not complete this task
- task-4 - Approve the synthetic release - by worker-2 - finished
- task-5 - File the synthetic report - by worker-1 - finished - completed task-5

## What it produced

- completed task-1
- completed task-2
- completed task-5

## Decisions queued

- decision-task-4 for item task-4 - Release the synthetic build tonight - answer approve or reject

## Counts

- backlog 5
- finished 4
- unfinished 1
- recorded 5

## Stock

- workers 2
- stocked at start 5
- stockouts recorded 0
```

## Install

Thirdshift is a Rust crate that builds from source with cargo.

```sh
git clone https://github.com/david-engelmann/thirdshift.git
cd thirdshift
cargo build
```

The binary lands at target/debug/thirdshift, and the test suite runs with `cargo test`.

## Quickstart

From the repository root, with the committed examples.

```sh
./target/debug/thirdshift brief --backlog examples/backlog.json
```

The command prints the morning brief at the top of this page.

## Usage

### run

`thirdshift run --backlog <file>` works the backlog and prints the run record as JSON, the same content the brief renders, with one entry per task, the outputs the run produced, the decisions it queued, and the counts. With `--result <file>` it also writes the record to that file. With `--window-seconds <seconds>` it bounds the run by a window on its own clock, and every item the window leaves unstarted is recorded as unfinished with the window as its reason, while an item already started keeps its worked outcome.

### brief

`thirdshift brief --backlog <file>` prints the morning brief in Markdown, the block at the top of this page. With `--md <file>` and `--json <file>` it writes the brief in both forms to files instead, and it takes `--window-seconds` with the same reading as run.

### enqueue

`thirdshift enqueue --file <file> --id <id> --title <title>` appends one task to a backlog file, with `--kind` and `--question` setting the fields a plain task leaves at their defaults. A second task with an id the file already holds is refused.

```console
$ ./target/debug/thirdshift enqueue --file backlog.json --id task-6 --title "Sweep the synthetic floor"
enqueued task-6
```

### resolve

`thirdshift resolve --brief <file> --resolutions <file>` folds a resolutions file of verdicts and notes into a brief JSON file and prints the resolved record, with the answers folded in, the answered decisions settled, and the run history untouched.

### carry

`thirdshift carry --backlog <file> --brief <file> --out <file>` joins a brief JSON record against its backlog and writes a carried backlog holding exactly the record's unfinished items, verbatim and in their original order, and prints that backlog. A brief that does not join exactly against the backlog is refused and nothing is written.

```console
$ ./target/debug/thirdshift carry --backlog examples/backlog.json --brief examples/windowed-brief.json --out carried.json
{
  "tasks": [
    {
      "id": "task-3",
      "kind": "fail",
      "question": "",
      "title": "Rebuild the synthetic index"
    },
    {
      "id": "task-5",
      "kind": "work",
      "question": "",
      "title": "File the synthetic report"
    }
  ]
}
```

Every command reads only the files it is handed. A file that is malformed or cannot be read refuses with one error line and exit code 2.

## Status

The tool is complete at the surface its contract describes, with no stubbed or held back part, and the full CI gate runs on every change. Round 10 landed in PR 21, guarding the help short on every command in the tree and closing the root table gap in the long pin. Round 11 landed in PR 23, firing the help battery behind carry in the committed corpus and closing the firing gap its review had recorded. Round 12 landed in PR 25, pinning the preprocess walk's branches by direct unit tests. Round 13 landed in PR 27, firing the carry command behind its states in the committed corpus. Round 14 landed in PR 29, staging the carry check order's unstaged precedence pairs in the committed corpus. Round 15 landed in PR 31, firing the resolve surface's content refusal families behind the resolve command in the committed corpus. Closed rounds 15 of 100. Part of the impatient programming portfolio, in the Fleet Operations arc.

## License

MIT. See LICENSE.
