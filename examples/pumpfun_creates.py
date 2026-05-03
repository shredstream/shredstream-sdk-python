import os
import sys
import time

import shredstream
from solders.pubkey import Pubkey
from solders.transaction import VersionedTransaction

PUMPFUN_PROGRAM_ID = Pubkey.from_string("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P")
CREATE_DISC = bytes([24, 30, 200, 40, 5, 28, 7, 119])
CREATE_V2_DISC = bytes([214, 144, 76, 236, 95, 139, 49, 180])


def detect_create(raw: bytes):
    try:
        tx = VersionedTransaction.from_bytes(raw)
    except Exception:
        return None
    msg = tx.message
    keys = list(msg.account_keys)
    for ix in msg.instructions:
        pid_idx = int(ix.program_id_index)
        if pid_idx >= len(keys):
            continue
        if keys[pid_idx] != PUMPFUN_PROGRAM_ID:
            continue
        data = bytes(ix.data)
        if len(data) < 8:
            continue
        disc = data[:8]
        if disc != CREATE_DISC and disc != CREATE_V2_DISC:
            continue
        is_v2 = disc == CREATE_V2_DISC

        def resolve(idx):
            if idx >= len(ix.accounts):
                return ""
            k = ix.accounts[idx]
            if k >= len(keys):
                return ""
            return str(keys[k])

        creator_idx = 5 if is_v2 else 7
        return {
            "mint": resolve(0),
            "bonding_curve": resolve(2),
            "creator": resolve(creator_idx),
            "sig": str(tx.signatures[0]),
        }
    return None


def print_card(slot: int, sig: str, create: dict):
    now = time.time()
    ms = int((now - int(now)) * 1000)
    t = time.strftime("%H:%M:%S", time.localtime(now))
    time_str = f"{t}.{ms:03d}"
    sig_short = f"{sig[:4]}...{sig[-4:]}" if len(sig) >= 8 else sig

    G = "\x1b[1;32m"
    DIM = "\x1b[90m"
    W = "\x1b[97m"
    Y = "\x1b[33m"
    C = "\x1b[36m"
    M = "\x1b[35m"
    D = "\x1b[2m"
    R = "\x1b[0m"

    print(f"{DIM}┌───────────────────────────────────────────────────────────────┐{R}")
    print(f"{DIM}│{R}  🌐 {W}ShredStream.com{R} {DIM}SDK{R}                                       {DIM}│{R}")
    print(f"{DIM}└───────────────────────────────────────────────────────────────┘{R}")
    print()
    print(f"{G}━━━━━━━━━━━━━━━━━━━━━━ 🚀 PUMPFUN CREATE ━━━━━━━━━━━━━━━━━━━━━━━{R}")
    print(f" {DIM}›{R} {DIM}🕐 Time{R}     {W}{time_str}{R}")
    print(f" {DIM}›{R} {DIM}📦 Slot{R}     {W}{slot}{R}")
    print(f" {DIM}›{R} {DIM}🪙 Mint{R}     {Y}{create['mint']}{R}")
    print(f" {DIM}›{R} {DIM}📈 Curve{R}    {C}{create['bonding_curve']}{R}")
    print(f" {DIM}›{R} {DIM}👤 Creator{R}  {M}{create['creator']}{R}")
    print(f" {DIM}›{R} {DIM}🔑 Sig{R}      {D}{sig_short}{R}")
    print(f"{G}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{R}")


def main() -> int:
    port = int(sys.argv[1]) if len(sys.argv) > 1 else int(os.environ.get("SHREDSTREAM_PORT", "8001"))
    listener = shredstream.ShredListener.bind(port)
    print(f"Listening for PumpFun creates on {listener.local_addr()}", file=sys.stderr)

    found = 0
    for slot, txs in listener:
        for raw in txs:
            create = detect_create(raw)
            if create is None:
                continue
            found += 1
            print("\x1b[H\x1b[2J", end="")
            print_card(slot, create["sig"], create)
            print(f"\n\x1b[90m  #{found} detected\x1b[0m")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
