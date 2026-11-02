"""The stocked queue, its workers, and its exclusive claims with leases."""

from dataclasses import dataclass, field


class StockoutError(Exception):
    """A worker asked for work and the queue held none. A recorded defect."""


class ClaimConflict(Exception):
    """A second claim was attempted on an item under a live claim."""


class DuplicateItem(Exception):
    """An item identifier was enqueued twice."""


@dataclass
class Item:
    item_id: str
    title: str
    kind: str = "work"
    question: str = ""

    def to_dict(self):
        return {
            "id": self.item_id,
            "title": self.title,
            "kind": self.kind,
            "question": self.question,
        }

    @classmethod
    def from_dict(cls, data):
        return cls(
            item_id=data["id"],
            title=data.get("title", ""),
            kind=data.get("kind", "work"),
            question=data.get("question", ""),
        )


@dataclass
class Claim:
    item_id: str
    worker_id: str
    expires_at: float


@dataclass
class WorkQueue:
    """A stocked queue. The clock is required and always injected by the
    caller, so no lease ever reads the wall clock by accident."""

    clock: object
    workers: list = field(default_factory=list)
    items: dict = field(default_factory=dict)
    order: list = field(default_factory=list)
    claims: dict = field(default_factory=dict)
    done: dict = field(default_factory=dict)
    stockouts: int = 0

    def register_worker(self, worker_id):
        if worker_id not in self.workers:
            self.workers.append(worker_id)
        return worker_id

    def enqueue(self, item):
        if item.item_id in self.items:
            raise DuplicateItem(item.item_id)
        self.items[item.item_id] = item
        self.order.append(item.item_id)
        return item

    def _claim_is_live(self, item_id):
        """A pure read. True while a claim exists and its lease is unexpired.

        This query never deletes anything. Expired claims are cleared only
        by _prune_expired_claim, which only mutating operations may call.
        """
        claim = self.claims.get(item_id)
        return claim is not None and self.clock.now() < claim.expires_at

    def _prune_expired_claim(self, item_id):
        claim = self.claims.get(item_id)
        if claim is not None and self.clock.now() >= claim.expires_at:
            del self.claims[item_id]

    def pending_ids(self):
        return [
            item_id
            for item_id in self.order
            if item_id not in self.done and not self._claim_is_live(item_id)
        ]

    def stock_report(self):
        stocked = len(self.pending_ids())
        workers = len(self.workers)
        deficit = max(0, workers - stocked)
        return {
            "workers": workers,
            "stocked": stocked,
            "deficit": deficit,
            "stockout": deficit > 0,
            "stockouts_recorded": self.stockouts,
        }

    @staticmethod
    def _check_lease(lease_seconds):
        if float(lease_seconds) <= 0:
            raise ValueError(
                "lease_seconds must be positive, got " + str(lease_seconds)
            )

    def claim(self, item_id, worker_id, lease_seconds=60.0):
        self._check_lease(lease_seconds)
        if item_id not in self.items or item_id in self.done:
            self.stockouts += 1
            raise StockoutError(item_id)
        self._prune_expired_claim(item_id)
        if item_id in self.claims:
            raise ClaimConflict(item_id)
        claim = Claim(
            item_id=item_id,
            worker_id=worker_id,
            expires_at=self.clock.now() + float(lease_seconds),
        )
        self.claims[item_id] = claim
        return claim

    def claim_next(self, worker_id, lease_seconds=60.0):
        self._check_lease(lease_seconds)
        pending = self.pending_ids()
        if not pending:
            self.stockouts += 1
            raise StockoutError(worker_id)
        return self.claim(pending[0], worker_id, lease_seconds=lease_seconds)

    def release(self, item_id, worker_id):
        self._prune_expired_claim(item_id)
        claim = self.claims.get(item_id)
        if claim is not None and claim.worker_id == worker_id:
            del self.claims[item_id]
            return True
        return False

    def complete(self, item_id, worker_id, outcome):
        claim = self.claims.get(item_id)
        if claim is None or claim.worker_id != worker_id:
            raise ClaimConflict(item_id)
        if self.clock.now() >= claim.expires_at:
            # The lease is gone, so the completion is refused and nothing
            # changes. The expired claim stays on record until a mutating
            # operation clears it.
            raise ClaimConflict(item_id)
        del self.claims[item_id]
        self.done[item_id] = outcome
        return outcome
