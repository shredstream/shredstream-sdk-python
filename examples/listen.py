"""Minimal example: bind a listener and print every transaction signature."""

import os

import shredstream
from solders.transaction import VersionedTransaction


def main() -> None:
    port = int(os.environ.get("SHREDSTREAM_PORT", "8001"))
    listener = shredstream.ShredListener.bind(port)
    print(f"listening on {listener.local_addr()}")
    for slot, txs in listener:
        for raw in txs:
            tx = VersionedTransaction.from_bytes(raw)
            print(f"slot={slot} sig={tx.signatures[0]}")


if __name__ == "__main__":
    main()
