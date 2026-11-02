"""Pins for TS-1 and TS-2, the stocked queue and the stockout defect."""

import pytest

from thirdshift import FakeClock, Item, StockoutError, WorkQueue


def stocked_queue(workers=2, items=3):
    queue = WorkQueue(clock=FakeClock())
    for index in range(workers):
        queue.register_worker("worker-" + str(index + 1))
    for index in range(items):
        queue.enqueue(Item(item_id="task-" + str(index + 1), title="synthetic task"))
    return queue


def test_stock_report_names_workers_stocked_and_deficit():
    report = stocked_queue(workers=2, items=3).stock_report()
    assert report["workers"] == 2
    assert report["stocked"] == 3
    assert report["deficit"] == 0
    assert report["stockout"] is False


def test_stock_report_shows_deficit_when_items_run_below_workers():
    report = stocked_queue(workers=3, items=1).stock_report()
    assert report["stocked"] == 1
    assert report["deficit"] == 2
    assert report["stockout"] is True


def test_claimed_items_no_longer_count_as_stocked():
    queue = stocked_queue(workers=2, items=2)
    queue.claim_next("worker-1")
    report = queue.stock_report()
    assert report["stocked"] == 1
    assert report["deficit"] == 1


def test_claim_next_on_empty_queue_raises_stockout_and_records_it():
    queue = stocked_queue(workers=1, items=0)
    with pytest.raises(StockoutError):
        queue.claim_next("worker-1")
    assert queue.stockouts == 1
    assert queue.stock_report()["stockouts_recorded"] == 1


def test_every_stockout_is_recorded_not_just_the_first():
    queue = stocked_queue(workers=1, items=0)
    for _ in range(3):
        with pytest.raises(StockoutError):
            queue.claim_next("worker-1")
    assert queue.stockouts == 3


def test_stockouts_from_direct_claims_are_recorded_in_the_same_counter():
    queue = stocked_queue(workers=1, items=1)
    queue.claim("task-1", "worker-1")
    queue.complete("task-1", "worker-1", "finished")
    with pytest.raises(StockoutError):
        queue.claim("task-1", "worker-1")
    with pytest.raises(StockoutError):
        queue.claim("task-missing", "worker-1")
    assert queue.stockouts == 2
    assert queue.stock_report()["stockouts_recorded"] == 2


def test_reading_the_queue_does_not_prune_expired_claims():
    clock = FakeClock()
    queue = WorkQueue(clock=clock)
    queue.register_worker("worker-1")
    queue.enqueue(Item(item_id="task-1", title="synthetic task"))
    queue.claim("task-1", "worker-1", lease_seconds=10.0)
    clock.advance(20.0)
    before = dict(queue.claims)
    report = queue.stock_report()
    pending = queue.pending_ids()
    assert queue.claims == before
    assert pending == ["task-1"]
    assert report["stocked"] == 1
