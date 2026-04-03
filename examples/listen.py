"""Minimal example: print transaction signatures as they arrive."""

from shredstream import ShredListener

for slot, transactions in ShredListener(port=8001):
    for tx in transactions:
        print(tx.signature)
