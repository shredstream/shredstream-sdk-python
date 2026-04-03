from __future__ import annotations

import struct
from dataclasses import dataclass

_SLOT_OFF = 0x41
_INDEX_OFF = 0x49
_FLAGS_OFF = 0x55
_SIZE_OFF = 0x56
DATA_HEADER_SIZE = 88

_DATA_COMPLETE = 0x40
_LAST_IN_SLOT = 0xC0


@dataclass(slots=True)
class ParsedShred:
    slot: int
    index: int
    payload: bytes
    batch_complete: bool
    last_in_slot: bool


def parse_shred(raw: bytes) -> ParsedShred | None:
    if len(raw) < DATA_HEADER_SIZE:
        return None

    slot = struct.unpack_from("<Q", raw, _SLOT_OFF)[0]
    index = struct.unpack_from("<I", raw, _INDEX_OFF)[0]
    flags = raw[_FLAGS_OFF]
    size = struct.unpack_from("<H", raw, _SIZE_OFF)[0]

    if size < DATA_HEADER_SIZE:
        return None

    if size > len(raw):
        return None
    payload = raw[DATA_HEADER_SIZE:size]

    last_in_slot = (flags & _LAST_IN_SLOT) == _LAST_IN_SLOT
    batch_complete = last_in_slot or (flags & _DATA_COMPLETE) != 0

    return ParsedShred(
        slot=slot,
        index=index,
        payload=payload,
        batch_complete=batch_complete,
        last_in_slot=last_in_slot,
    )
