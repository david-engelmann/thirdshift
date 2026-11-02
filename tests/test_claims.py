"""Pins for TS-3 and TS-4, exclusive claims and lease expiry on a fake clock."""

import pytest

from thirdshift import ClaimConflict, FakeClock, Item, WorkQueue


def one_item_queue():
    clock = FakeClock(start=100.0)
    queue = WorkQueue(clock=clock)
    queue.register_worker("worker-1")
    queue.register_worker("worker-2")
    queue.enqueue(Item(item_id="task-1", title="synthetic task"))
    return clock, queue


def test_second_claim_on_live_claim_conflicts_and_changes_nothing():
    _, queue = one_item_queue()
    first = queue.claim("task-1", "worker-1", lease_seconds=30.0)
    with pytest.raises(ClaimConflict):
        queue.claim("task-1", "worker-2", lease_seconds=30.0)
    assert queue.claims["task-1"].worker_id == "worker-1"
    assert queue.claims["task-1"].expires_at == first.expires_at


def test_claim_expires_at_the_lease_boundary_on_the_injected_clock():
    clock, queue = one_item_queue()
    queue.claim("task-1", "worker-1", lease_seconds=30.0)
    clock.advance(29.0)
    with pytest.raises(ClaimConflict):
        queue.claim("task-1", "worker-2", lease_seconds=30.0)
    clock.advance(1.0)
    claim = queue.claim("task-1", "worker-2", lease_seconds=30.0)
    assert claim.worker_id == "worker-2"


def test_release_frees_the_item_for_another_worker_at_once():
    _, queue = one_item_queue()
    queue.claim("task-1", "worker-1", lease_seconds=30.0)
    assert queue.release("task-1", "worker-1") is True
    claim = queue.claim("task-1", "worker-2", lease_seconds=30.0)
    assert claim.worker_id == "worker-2"


def test_release_by_a_worker_who_does_not_hold_the_item_frees_nothing():
    _, queue = one_item_queue()
    queue.claim("task-1", "worker-1", lease_seconds=30.0)
    assert queue.release("task-1", "worker-2") is False
    with pytest.raises(ClaimConflict):
        queue.claim("task-1", "worker-2", lease_seconds=30.0)


def test_complete_requires_the_holding_worker():
    _, queue = one_item_queue()
    queue.claim("task-1", "worker-1", lease_seconds=30.0)
    with pytest.raises(ClaimConflict):
        queue.complete("task-1", "worker-2", "finished")
    queue.complete("task-1", "worker-1", "finished")
    assert queue.pending_ids() == []


def test_completion_after_the_lease_expires_is_refused_and_changes_nothing():
    clock, queue = one_item_queue()
    queue.claim("task-1", "worker-1", lease_seconds=30.0)
    clock.advance(31.0)
    with pytest.raises(ClaimConflict):
        queue.complete("task-1", "worker-1", "finished")
    assert queue.done == {}
    assert queue.claims["task-1"].worker_id == "worker-1"
    queue.claim("task-1", "worker-2", lease_seconds=30.0)
    queue.complete("task-1", "worker-2", "finished")
    assert queue.done == {"task-1": "finished"}


def test_completion_at_the_exact_lease_boundary_is_refused():
    clock, queue = one_item_queue()
    queue.claim("task-1", "worker-1", lease_seconds=30.0)
    clock.advance(30.0)
    with pytest.raises(ClaimConflict):
        queue.complete("task-1", "worker-1", "finished")
    assert queue.done == {}


@pytest.mark.parametrize("lease", [0, -50.0])
def test_non_positive_leases_are_rejected_before_any_state_changes(lease):
    _, queue = one_item_queue()
    with pytest.raises(ValueError):
        queue.claim("task-1", "worker-1", lease_seconds=lease)
    assert queue.claims == {}
    with pytest.raises(ValueError):
        queue.claim_next("worker-1", lease_seconds=lease)


def test_a_queue_cannot_be_built_without_a_clock():
    with pytest.raises(TypeError):
        WorkQueue()


def test_claim_next_hands_out_items_in_enqueue_order():
    clock = FakeClock()
    queue = WorkQueue(clock=clock)
    queue.register_worker("worker-1")
    queue.enqueue(Item(item_id="task-b", title="second enqueued"))
    queue.enqueue(Item(item_id="task-a", title="first in name only"))
    assert queue.claim_next("worker-1").item_id == "task-b"
