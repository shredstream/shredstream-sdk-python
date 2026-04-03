from __future__ import annotations

import socket
from typing import Iterator

from shredstream.accumulator import SlotAccumulator
from shredstream.decoder import Transaction
from shredstream.parser import ParsedShred, parse_shred


class ShredListener:

    def __init__(
        self,
        port: int = 8001,
        recv_buf: int = 25 * 1024 * 1024,
        max_age: int = 10,
    ) -> None:
        self._port = port
        self._max_age = max_age
        self._sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self._sock.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, recv_buf)
        self._sock.bind(("0.0.0.0", port))

        self._slots: dict[int, SlotAccumulator] = {}
        self._highest_slot: int = 0

    def __iter__(self) -> ShredListener:
        return self

    def __next__(self) -> tuple[int, list[Transaction]]:
        while True:
            data = self._sock.recv(2048)

            shred = parse_shred(data)
            if shred is None:
                continue

            txs = self._process_shred(shred)
            if txs:
                return shred.slot, txs

    def shreds(self) -> Iterator[ParsedShred]:
        while True:
            data = self._sock.recv(2048)
            shred = parse_shred(data)
            if shred is not None:
                yield shred

    def stop(self) -> None:
        self._sock.close()

    def _process_shred(self, shred: ParsedShred) -> list[Transaction]:
        slot = shred.slot

        if slot > self._highest_slot:
            self._highest_slot = slot
            self._evict_old_slots()

        if slot not in self._slots:
            self._slots[slot] = SlotAccumulator()

        acc = self._slots[slot]
        prev_errors = acc.decode_errors
        txs = acc.push(shred.index, shred.payload, shred.batch_complete, shred.last_in_slot)
        new_errors = acc.decode_errors - prev_errors
        if new_errors > 0:
            del self._slots[slot]
            return txs

        if acc.slot_complete:
            del self._slots[slot]

        return txs

    def _evict_old_slots(self) -> None:
        cutoff = self._highest_slot - self._max_age
        stale = [s for s in self._slots if s < cutoff]
        for s in stale:
            del self._slots[s]
