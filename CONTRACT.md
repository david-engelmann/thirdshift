# Contract - thirdshift

This contract is the executable spec for thirdshift. Every obligation below is stated so it can fail. The full contract is the first build act of this repo and it extends the stub obligations written here.

## Stub obligations

TS-S1. The queue holds work ahead of every registered worker, so a worker that asks for work and finds an empty queue is a defect the tests must catch.

TS-S2. A work item is held by at most one worker at a time under a claim, and a second claim on a held item fails until the claim is released or expires.

TS-S3. A supervised run over a test backlog finishes every item it claims or records why it did not, and it never silently drops an item.

TS-S4. The morning brief lists what ran, what it produced, and every decision queued for the operator, and each queued decision can be approved or rejected from the brief data alone.

TS-S5. Every fixture and example in this repo is synthetic and built for this repo, and no real trace or session data appears anywhere in the tree.
