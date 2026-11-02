# Contract - thirdshift

This contract is the executable spec for thirdshift. Every obligation below is stated so it can fail. The full contract is the first build act of this repo and it extends the stub obligations written here.

## Stub obligations

TS-S1. The queue holds work ahead of every registered worker, so a worker that asks for work and finds an empty queue is a defect the tests must catch.

TS-S2. A work item is held by at most one worker at a time under a claim, and a second claim on a held item fails until the claim is released or expires.

TS-S3. A supervised run over a test backlog finishes every item it claims or records why it did not, and it never silently drops an item.

TS-S4. The morning brief lists what ran, what it produced, and every decision queued for the operator, and each queued decision can be approved or rejected from the brief data alone.

TS-S5. Every fixture and example in this repo is synthetic and built for this repo, and no real trace or session data appears anywhere in the tree.

## Round 1 obligations

The obligations below make the stub obligations executable. TS-1 and TS-2 make TS-S1 executable. TS-3 and TS-4 make TS-S2 executable. TS-5 and TS-6 make TS-S3 executable. TS-7 and TS-8 make TS-S4 executable. TS-9 and TS-10 make TS-S5 and the honesty of the whole slice executable.

TS-1. The queue is stocked when the number of pending and unclaimed items is at least the number of registered workers, and the queue reports its state through a stock report that names the worker count, the stocked count, the deficit, and whether a stockout stands. A stock report that hides a deficit is a defect.

TS-2. When a registered worker asks for the next item and no pending and unclaimed item exists, the queue raises a stockout error and records the stockout in its state, so an idle worker is always a recorded defect and never a silent wait.

TS-3. A claim holds an item for exactly one worker. A second claim on an item whose claim is still live fails with a claim conflict, and the failed claim changes no state, so the first holder keeps the item and the item appears in no second claim record.

TS-4. Every claim carries a lease that expires at a time read from a clock the caller injects. Once the clock passes the expiry, another worker may claim the item. Releasing a live claim also frees the item at once. No code path reads wall time directly, and no test sleeps to wait out a lease.

TS-5. The runner works a backlog to a record. Every item in the backlog appears exactly once in the run record as finished with its output or as unfinished with a reason, and the counts in the record add up to the size of the backlog, so an item the runner dropped silently would break the record.

TS-6. The runner takes work only through claims on the queue, registers its workers before it starts, and names the claiming worker on every run record entry, so work done outside the claim discipline cannot hide in the record.

TS-7. A backlog task marked as a decision task finishes by queueing a decision rather than by producing work output. Every queued decision carries an identifier, a question, and the two responses approve and reject, so the operator can answer from the decision record alone.

TS-8. The morning brief exists as Markdown and as JSON with the same content. Both forms list what ran with the worker and the outcome for each item, what the run produced, and every queued decision with its identifier and question and both responses, so anything readable in one form is present in the other.

TS-9. A run over the same backlog with the same starting clock produces byte identical brief files, because the run records no wall time and orders every list by item identifier, so a brief can be regenerated and diffed without noise.

TS-10. The package depends on the Python standard library only, and every fixture, backlog, and example in the repo is synthetic and written for this repo, so the slice runs offline with nothing installed beyond Python and a test runner.

## Round 1 review amendments

The obligations below amend the Round 1 obligations after the independent red team and two axis review of the first build. The earlier text is left standing and these obligations govern where the two disagree.

TS-11. A completion is accepted only while the claim lease is still live on the injected clock. Once the lease has expired, completion by the former holder fails with a claim conflict and changes no state, whether or not another worker has claimed the item in the meantime.

TS-12. A claim with a lease of zero or negative seconds is rejected with a clear error before any state changes, so a stillborn lease can never be handed out.

TS-13. Reading the queue never changes it. The stock report and the pending list are computed from the claims on record without deleting any of them, and expired claims are cleared only inside operations that change the queue, such as claiming, releasing, or completing.

TS-14. Every stockout is recorded, whichever path raises it. A worker that claims a named item and finds it missing or already done raises the stockout error and adds to the same recorded stockout count as a worker that asks for the next item and finds none waiting.

TS-15. A backlog that names the same item identifier twice still runs to a full record. The first usable entry with an identifier is worked normally, and every later usable entry with that identifier is recorded as unfinished with a reason that names the duplication, so the counts in the record still add up to the size of the backlog.

TS-16. A backlog entry that cannot become a task, because it is not an object or because it carries no identifier, is recorded as unfinished with a reason that names the defect, and the run works the remaining entries instead of failing. An entry that carries only an identifier still runs.

TS-17. A task that carries a question is a decision or it is malformed. When the kind is missing or is anything other than decision while a question is present, the entry is recorded as unfinished with a reason that names the malformed decision, and its question is never dropped by running it as work. A decision task with no question falls back to its title as the question.

TS-18. The two brief forms carry the same content without exception. The Markdown brief renders the stock block, the title of every item that ran, and the item behind every decision, alongside everything the JSON form carries, so a reader of either form sees the whole brief.

TS-19. Worker attribution is decided by item identifier order, never by the order entries appear in the backlog. The runner sorts the work by identifier before it assigns workers, so the same backlog in a different entry order produces byte identical briefs.

TS-20. The stock block in a brief reports the run itself. It names the workers on the run, the items stocked when the run started, and the stockouts recorded during the run, and it never reports a standing stockout for a run that finished its backlog. The live queue keeps its own stock report under TS-1 and TS-2.

TS-21. A backlog file that cannot be read as a backlog fails at load with a clear error. Invalid JSON and an object with no tasks list are load errors, the command line prints the error to standard error and exits with status 2, and no traceback is shown. A run asked to work a non-empty backlog with no workers fails with a clear validation error before any item is touched.

TS-22. The queue takes its clock from its caller and never defaults to the wall clock. Building a queue without a clock is an error, so every lease in every run reads time from a clock somebody chose.

## Round 2 obligations

The obligations below make the second half of TS-S4 executable. Round 1 queues the decisions in the brief, and these obligations record the answers and fold them back into the record. TS-23 states the resolutions file. TS-24 states what applying a resolutions file produces. TS-25 states the refusals and makes application all or nothing. TS-26 keeps the run record itself untouched. TS-27 puts the step on the command line.

TS-23. A resolutions file is a JSON object with a resolutions key, and the value of that key is a list of resolution objects. Each resolution object carries the key decision with the identifier of one queued decision, the key verdict with the word approve or the word reject, and the key note with the note the operator wrote. A resolutions file that cannot be read, is not valid JSON, is not an object, or carries no resolutions list fails at load with a clear error. A resolution object that is not an object, or that misses one of the three keys, or that carries one of them with the wrong type, is refused with a clear error.

TS-24. Applying a resolutions list to a brief record returns a resolved record. Every decision the resolutions name appears exactly once in the list under the key resolved, in the order the decisions stood in the brief, and each entry there carries the decision identifier, the item, the question, the verdict, and the note. Every decision the resolutions do not name stays in the list under the key decisions exactly as the brief carried it. The resolved record carries a block under the key decision_counts with the keys carried, queued, and resolved, and the queued and resolved values add up to the carried value.

TS-25. Application is all or nothing. A resolution that names a decision the brief does not carry refuses the whole application. A resolutions list that names the same decision twice refuses the whole application. A verdict that is neither approve nor reject refuses the whole application. Each refusal is a clear error that names the defect, no resolved record is produced, and the brief record is left exactly as it was.

TS-26. Resolving never rewrites the run. The ran entries, the produced list, the counts block, and the stock block of the resolved record carry the same values the brief record carried, and the brief record handed to the resolver is unchanged after the call, whether the call succeeds or refuses. The resolved record adds the resolved list and the decision counts block alongside the run record and takes nothing away from it.

TS-27. The command line carries a resolve step beside the brief step. The step takes the path of a brief JSON file and the path of a resolutions file and prints the resolved record as JSON on standard output. A brief file or a resolutions file that cannot be read, is not valid JSON, is not the shape its format requires, or fails the refusals of TS-25 prints one error line to standard error and exits with status 2, and no traceback is shown.

TS-28. Amendment, added by the Round 2 review. The shape a brief record must carry for the resolve step is stated exactly. Every entry in its decisions list is an object carrying a non-empty string id and string item and question fields, the shape the brief step writes, and no decision id repeats in the list. A record outside that shape is refused whole under the same discipline as TS-25 and TS-27, with the error naming the defect, so no decision can fold twice and no malformed entry can ride through unresolved. The resolved record the step returns is a deep copy of the record it was resolved from, so it shares no mutable state with its input.

## Round 3 obligations

The obligations below give a run the window this fleet works through. TS-29 states the window parameter and how it gates the starting of work. TS-30 states the record of an item the window left unstarted. TS-31 states the window block the brief carries. TS-32 puts the window on the command line and keeps the resolve step composing with it.

TS-29. run_backlog gains an optional window parameter named window. The window is a non-negative number of clock seconds measured from the start of the run on the clock the run itself uses. Before the runner starts work on each next item it reads that clock. When the elapsed time since the run started has reached the window the runner starts no further item. An item already started is worked to its recorded outcome exactly as before even when the clock passes the end of the window while it is worked because the window gates starting and never interrupts work in progress. A window of zero starts no item. A negative window and a window that is not a number are refused with a clear error before any item is touched. Giving no window leaves the behaviour of the run byte identical to the run Round 2 produced.

TS-30. Every backlog item the runner never started because the window closed is recorded in the run record as unfinished with the reason "the window closed before this item was started" stated word for word wherever that reason appears. Such an item was never claimed so the entry carries no worker and the item carries no claim and no lease and the title of the item still appears in the record. An unstarted item is not a stockout and it is not a completion and the stock block records no stockout for it and the counts of the record count it as unfinished while the finished count and the produced list gain nothing from it. A decision item left unstarted queues no decision.

TS-31. Where a run was given a window the run record carries a block under the key window and the brief carries that block in both its forms. The block carries the key window_seconds with the length of the window and the key elapsed_seconds with the elapsed clock time the run consumed and the key started with how many items the runner started inside the window and the key unstarted with how many items were left unstarted because the window closed. The Markdown brief renders a window section carrying those same four values so a reader of either form sees the same window content. Where no window was given the record and the brief carry no window block and the brief stays byte identical to the brief Round 2 produced.

TS-32. The run step and the brief step of the command line each gain an option named --window-seconds that takes the window in seconds. A window that is negative or that is not a number prints one error line to standard error and exits with status 2 and shows no traceback on either step. The option composes with the resolve step unchanged because the resolve step reads the brief the run produced and a resolved record built from a windowed brief carries the window block across unchanged.
