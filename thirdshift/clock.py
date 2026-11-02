"""Clocks. Every lease reads time from an injected clock, never from the wall."""

import time


class SystemClock:
    """The production clock. Tests use FakeClock instead."""

    def now(self):
        return time.time()


class FakeClock:
    """A deterministic clock that moves only when told to."""

    def __init__(self, start=0.0):
        self._now = float(start)

    def now(self):
        return self._now

    def advance(self, seconds):
        self._now += float(seconds)
        return self._now
