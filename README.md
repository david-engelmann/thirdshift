# thirdshift

The fleet's third shift in one repo, a scheduler that keeps work queued ahead of every worker so no agent ever idles, works the backlog through the night, and leaves a morning brief with the decisions it queued for you.

## What this is

Unqueued work dies overnight, because the runtimes hand the operator nothing when a session ends and accept no mid-run input, so a fleet that is not fed simply stops. Thirdshift is the operating layer of the overnight fleet. The queue is always stocked ahead of the workers, because an agent asking what next is an operator failure, and the night watch works the backlog under claim leases and cutoff discipline while the human sleeps. The morning brief is its human surface, what the fleet did, what it cost, and the decisions waiting with approve and reject on each. The boundary with its sibling is one sentence. Paceghost is the race against the window, thirdshift is the fleet that works through it.

## Status

Closed rounds 3 of 100. Round 2 landed in PR 4 and is recorded closed here. Round 3 landed in PR 6 and is recorded closed here, and it adds a run window under contract obligations TS-29 to TS-33, so a run can be bounded by a window of seconds on its own clock and every item the window leaves unstarted is recorded in the brief with its reason while an item already started keeps its worked outcome. It adds decision resolution under contract obligations TS-23 to TS-28, so the operator answers the decisions the brief queued from a recorded resolutions file of verdicts and notes, and applying that file returns a resolved record with the answers folded in and the run history untouched. The package carries a stocked queue with exclusive leased claims on an injected clock, a runner that works a synthetic backlog to a full record with nothing dropped silently, and a morning brief in Markdown and JSON with the same content, all under contract obligations TS-1 to TS-10, the review amendments TS-11 to TS-22, the Round 2 obligations TS-23 to TS-28, and the Round 3 obligations TS-29 to TS-33. Part of the impatient programming portfolio, in the Fleet Operations arc.

## Licence

MIT. See LICENSE.
