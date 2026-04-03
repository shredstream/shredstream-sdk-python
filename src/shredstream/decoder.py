from __future__ import annotations

import struct
from dataclasses import dataclass, field


@dataclass(slots=True)
class Transaction:
    signatures: list[bytes]
    raw: bytes

    @property
    def signature(self) -> str:
        from solders.signature import Signature

        return str(Signature.from_bytes(self.signatures[0]))


def _read_compact_u16(buf: bytes | memoryview, off: int) -> tuple[int, int]:
    b0 = buf[off]
    if b0 < 0x80:
        return b0, 1
    b1 = buf[off + 1]
    if b1 < 0x80:
        return (b0 & 0x7F) | (b1 << 7), 2
    b2 = buf[off + 2]
    return (b0 & 0x7F) | ((b1 & 0x7F) << 7) | (b2 << 14), 3


def _try_read_compact_u16(
    buf: bytes | memoryview, off: int, length: int
) -> tuple[int | None, int]:
    if off >= length:
        return None, 0
    b0 = buf[off]
    if b0 < 0x80:
        return b0, 1
    if off + 1 >= length:
        return None, 0
    b1 = buf[off + 1]
    if b1 < 0x80:
        return (b0 & 0x7F) | (b1 << 7), 2
    if off + 2 >= length:
        return None, 0
    b2 = buf[off + 2]
    return (b0 & 0x7F) | ((b1 & 0x7F) << 7) | (b2 << 14), 3


def _try_parse_transaction(
    buf: bytes | memoryview, off: int, length: int
) -> tuple[int, list[bytes]] | None:
    sig_count, consumed = _try_read_compact_u16(buf, off, length)
    if sig_count is None:
        return None
    off += consumed

    sigs: list[bytes] = []
    for _ in range(sig_count):
        if off + 64 > length:
            return None
        sigs.append(bytes(buf[off : off + 64]))
        off += 64

    if off >= length:
        return None
    first_byte = buf[off]
    is_v0 = first_byte >= 0x80
    if is_v0:
        off += 1

    off += 3
    if off > length:
        return None

    acct_count, consumed = _try_read_compact_u16(buf, off, length)
    if acct_count is None:
        return None
    off += consumed
    off += acct_count * 32
    if off > length:
        return None

    off += 32
    if off > length:
        return None

    ix_count, consumed = _try_read_compact_u16(buf, off, length)
    if ix_count is None:
        return None
    off += consumed
    for _ in range(ix_count):
        off += 1
        if off > length:
            return None
        accts_len, consumed = _try_read_compact_u16(buf, off, length)
        if accts_len is None:
            return None
        off += consumed + accts_len
        if off > length:
            return None
        data_len, consumed = _try_read_compact_u16(buf, off, length)
        if data_len is None:
            return None
        off += consumed + data_len
        if off > length:
            return None

    if is_v0:
        lookups_count, consumed = _try_read_compact_u16(buf, off, length)
        if lookups_count is None:
            return None
        off += consumed
        for _ in range(lookups_count):
            off += 32
            if off > length:
                return None
            writable_len, consumed = _try_read_compact_u16(buf, off, length)
            if writable_len is None:
                return None
            off += consumed + writable_len
            if off > length:
                return None
            readonly_len, consumed = _try_read_compact_u16(buf, off, length)
            if readonly_len is None:
                return None
            off += consumed + readonly_len
            if off > length:
                return None

    return off, sigs


_parse_transaction = _try_parse_transaction


class BatchDecoder:

    def __init__(self) -> None:
        self._buf = bytearray()
        self._cursor: int = 0
        self._expected_count: int | None = None
        self._entries_yielded: int = 0
        self._last_error: bool = False

    def push(self, payload: bytes) -> list[Transaction]:
        self._last_error = False
        self._buf.extend(payload)
        return self._drain()

    @property
    def had_error(self) -> bool:
        return self._last_error

    def reset(self) -> None:
        self._buf.clear()
        self._cursor = 0
        self._expected_count = None
        self._entries_yielded = 0
        self._last_error = False

    def _drain(self) -> list[Transaction]:
        buf = memoryview(self._buf)
        length = len(buf)

        if self._expected_count is None:
            if length < self._cursor + 8:
                return []
            count = struct.unpack_from("<Q", buf, self._cursor)[0]
            self._cursor += 8
            if count > 100_000:
                self._last_error = True
                return []
            self._expected_count = count

        transactions: list[Transaction] = []

        while self._entries_yielded < self._expected_count:
            if self._cursor + 48 > length:
                break

            off = self._cursor
            off += 8 + 32
            if off + 8 > length:
                break
            tx_count = struct.unpack_from("<Q", buf, off)[0]
            off += 8

            entry_txs: list[Transaction] = []
            for _ in range(tx_count):
                tx_start = off
                result = _try_parse_transaction(buf, off, length)
                if result is None:
                    return transactions
                off, sigs = result
                entry_txs.append(
                    Transaction(signatures=sigs, raw=bytes(buf[tx_start:off]))
                )

            transactions.extend(entry_txs)
            self._cursor = off
            self._entries_yielded += 1

        return transactions
