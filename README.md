# thirdshift

The fleet's third shift in one repo, a scheduler that keeps work queued ahead of every worker so no agent ever idles, works the backlog through the night, and leaves a morning brief with the decisions it queued for you.

## What this is

Unqueued work dies overnight, because the runtimes hand the operator nothing when a session ends and accept no mid-run input, so a fleet that is not fed simply stops. Thirdshift is the operating layer of the overnight fleet. The queue is always stocked ahead of the workers, because an agent asking what next is an operator failure, and the night watch works the backlog under claim leases and cutoff discipline while the human sleeps. The morning brief is its human surface, what the fleet did, what it cost, and the decisions waiting with approve and reject on each. The boundary with its sibling is one sentence. Paceghost is the race against the window, thirdshift is the fleet that works through it.

## Status

Round 1 is built and in review. The package carries a stocked queue with exclusive leased claims, a runner that works a synthetic backlog to a full record, and a morning brief in Markdown and JSON, all under contract obligations TS-1 to TS-10 and the review amendments TS-11 to TS-22 that follow them. The fixes for the review findings have landed and the contract is not recorded as met until the owner reviews pass on the fixed build. Closed rounds 0 of 100, because a round closes only after the owner reviews pass. Part of the impatient programming portfolio, in the Fleet Operations arc.

## Licence

MIT. See LICENSE.
