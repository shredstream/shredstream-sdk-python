# Solana ShredStream SDK for Python

Solana ShredStream SDK/Decoder for Python, enabling ultra-low latency Solana transaction streaming via UDP shreds from ShredStream.com

> Part of the [ShredStream.com](https://shredstream.com) ecosystem — ultra-low latency [Solana shred streaming](https://shredstream.com) via UDP.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Python](https://img.shields.io/badge/Python-3.10+-3776AB?logo=python&logoColor=white)](#)

## 📋 Prerequisites

1. **Create an account** on [ShredStream.com](https://shredstream.com)
2. **Launch a Shred Stream** and pick your region (Frankfurt, Amsterdam, Singapore, Chicago, and more)
3. **Enter your server's IP address** and the UDP port where you want to receive shreds
4. **Open your firewall** for inbound UDP traffic on that port (e.g. configure your cloud provider's security group)
5. Install [Python 3.10+](https://python.org):
   ```bash
   # Linux (Ubuntu/Debian)
   sudo apt update && sudo apt install -y python3 python3-venv python3-pip

   # macOS
   brew install python3
   ```

> 🎁 Want to try before you buy? Open a ticket on our [Discord](https://discord.gg/4w2DNbTaWD) to request a free trial.

## 📦 Installation

```bash
# Create a virtual environment (recommended)
python3 -m venv venv
source venv/bin/activate

# Install the SDK
pip install shredstream
```

## ⚡ Quick Start

Create a file `main.py`:

```python
from shredstream import ShredListener
import os

# Bind to the UDP port configured on ShredStream.com
PORT = int(os.environ.get("SHREDSTREAM_PORT", 8001))
listener = ShredListener(port=PORT)

# Decoded transactions — ready-to-use Solana transactions
for slot, transactions in listener:
    for tx in transactions:
        print(f"slot {slot}: {tx.signature}")

# OR raw shreds — lowest latency, arrives before block assembly
# for shred in listener.shreds():
#     print(f"slot={shred.slot} index={shred.index} len={len(shred.payload)}")
```

Run it:

```bash
python3 main.py
```

## 📖 API Reference

### `ShredListener`

```python
ShredListener(port=8001, recv_buf=25*1024*1024, max_age=10)
```

| Parameter  | Type  | Default | Description                      |
|------------|-------|---------|----------------------------------|
| `port`     | `int` | 8001    | UDP port to bind                 |
| `recv_buf` | `int` | 25 MB   | Socket receive buffer size       |
| `max_age`  | `int` | 10      | Maximum slot age before eviction |

#### Methods

- **Iterator protocol** -- `for slot, transactions in listener:` yields decoded transactions as they arrive.
- `listener.shreds()` -- Generator yielding individual `ParsedShred` objects.
- `listener.active_slots()` -- Number of slots currently being accumulated.
- `listener.stop()` -- Closes the UDP socket.

### `ParsedShred`

| Field            | Type    | Description                            |
|------------------|---------|----------------------------------------|
| `slot`           | `int`   | Slot number                            |
| `index`          | `int`   | Shred index within the slot            |
| `payload`        | `bytes` | Raw shred payload (after header)       |
| `batch_complete` | `bool`  | True if this shred ends an entry batch |
| `last_in_slot`   | `bool`  | True if this is the last shred in slot |

### `Transaction`

| Field        | Type             | Description                                       |
|--------------|------------------|---------------------------------------------------|
| `signatures` | `list[bytes]`    | Raw 64-byte signatures                            |
| `raw`        | `bytes`          | Full wire-format transaction bytes                 |
| `signature`  | `str` (property) | First signature as base58 (lazy, via `solders`)    |

## 🎯 Use Cases

ShredStream.com shred data powers a wide range of latency-sensitive strategies — HFT, MEV extraction, token sniping, copy trading, liquidation bots, on-chain analytics, and more.

### 💎 PumpFun Token Sniping

ShredStream.com SDK detects PumpFun token creations **~499ms before they appear on PumpFun's live feed** — tested across 25 consecutive detections:

<img src="https://raw.githubusercontent.com/shredstream/shredstream-sdk-python/main/assets/shredstream.com_sdk_vs_pumpfun_live_feed.gif" alt="ShredStream.com SDK vs PumpFun live feed — ~499ms advantage" width="600">

> [ShredStream.com](https://shredstream.com) provides a complete, optimized PumpFun token creation detection code exclusively to Pro plan subscribers and above. Battle-tested, high-performance, ready to plug into your sniping pipeline. To get access, open a ticket on [Discord](https://discord.gg/4w2DNbTaWD) or reach out on Telegram [@shredstream](https://t.me/shredstream).

## ⚙️ Configuration

### OS Tuning

```bash
# Linux -- increase max receive buffer
sudo sysctl -w net.core.rmem_max=33554432

# macOS
sudo sysctl -w kern.ipc.maxsockbuf=33554432
```

### Dependencies

- `solders>=0.21` -- Required for base58 signature encoding (`tx.signature` property). Imported lazily on first access.

## 💡 Examples

### Filter by program

```python
from shredstream import ShredListener
from solders.pubkey import Pubkey

PUMP_FUN = bytes(Pubkey.from_string("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"))

for slot, txs in ShredListener(port=8001):
    for tx in txs:
        if PUMP_FUN in tx.raw:
            print(f"slot {slot}: {tx.signature}")
```

### Raw shred access

```python
from shredstream import ShredListener

listener = ShredListener(port=8001)
for shred in listener.shreds():
    print(f"slot={shred.slot} index={shred.index} len={len(shred.payload)}")
```

## 🚀 Launch a Shred Stream

Need a feed? **[Launch a Solana Shred Stream on ShredStream.com](https://shredstream.com)** — sub-millisecond delivery, multiple global regions, 5-minute setup.

## 🔗 Links

- 🌐 Website: https://www.shredstream.com/
- 📖 Documentation: https://docs.shredstream.com/
- 🐦 X (Twitter): https://x.com/ShredStream
- 🎮 Discord: https://discord.gg/4w2DNbTaWD
- 💬 Telegram: https://t.me/ShredStream
- 💻 GitHub: https://github.com/ShredStream
- 🎫 Support: [Discord](https://discord.gg/4w2DNbTaWD)
- 📊 Benchmarks: [Discord](https://discord.gg/4w2DNbTaWD)

## 📄 License

MIT — [ShredStream.com](https://shredstream.com)
