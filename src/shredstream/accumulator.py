from __future__ import annotations

from shredstream.decoder import BatchDecoder, Transaction

_GAP_SKIP_THRESHOLD = 5


class SlotAccumulator:

    def __init__(self) -> None:
        self._pending: dict[int, tuple[bytes, bool, bool]] = {}
        self._next_index: int = 0
        self._decoder = BatchDecoder()
        self._slot_complete: bool = False
        self._stall_count: int = 0
        self._decode_errors: int = 0

    @property
    def slot_complete(self) -> bool:
        return self._slot_complete

    @property
    def decode_errors(self) -> int:
        return self._decode_errors

    def push(
        self,
        index: int,
        payload: bytes,
        batch_complete: bool,
        last_in_slot: bool,
    ) -> list[Transaction]:
        if index in self._pending or index < self._next_index:
            return []

        self._pending[index] = (payload, batch_complete, last_in_slot)
        return self._drain()

    def _drain(self) -> list[Transaction]:
        all_txs: list[Transaction] = []
        drained_any = False

        while self._next_index in self._pending:
            drained_any = True
            payload, batch_complete, last_in_slot = self._pending.pop(self._next_index)
            self._next_index += 1

            txs = self._decoder.push(payload)
            if self._decoder.had_error:
                self._decode_errors += 1
                return all_txs

            all_txs.extend(txs)

            if last_in_slot:
                self._slot_complete = True

            if batch_complete:
                self._decoder.reset()

        if drained_any:
            self._stall_count = 0
        else:
            self._stall_count += 1
            if self._stall_count >= _GAP_SKIP_THRESHOLD and self._pending:
                self._next_index = min(self._pending)
                self._stall_count = 0
                self._decoder.reset()
                return self._drain()

        return all_txs
