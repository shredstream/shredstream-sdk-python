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
from solders.transaction import VersionedTransaction
import os

PORT = int(os.environ.get("SHREDSTREAM_PORT", 8001))
listener = ShredListener.bind(PORT)

# Decoded transactions — ready-to-use Solana transactions
for slot, txs in listener:
    for raw in txs:
        tx = VersionedTransaction.from_bytes(raw)
        print(f"slot {slot}: {tx.signatures[0]}")
```

Each `raw` is the bincode wire-format encoding of a `VersionedTransaction`.

Run it:

```bash
python3 main.py
```

## 📖 API Reference

### `ShredListener`

- `ShredListener.bind(port: int) -> ShredListener` — Bind with defaults (64 MB recv buf, 3 slot window, FEC enabled)
- `ShredListener.bind_with_options(port, options: ListenerOptions) -> ShredListener` — Custom configuration
- `ShredListener.offline() -> ShredListener` — Bind on ephemeral port; drive via `handle_packet`
- `ShredListener.from_fd(fd: int, options=None) -> ShredListener` — Adopt an existing UDP file descriptor
- **Iterator protocol** — `for slot, txs in listener:` yields `(int, list[bytes])`
- `listener.shreds() -> ShredIter` — Iterator yielding `RawShred` headers (no decode)
- `listener.handle_packet(data: bytes) -> tuple[int, list[bytes]] | None` — Inject an externally-received UDP datagram
- `listener.local_addr() -> str` — Bound socket address
- `listener.close()` — Release the socket

### `ListenerOptions`

```python
ListenerOptions(
    recv_buf=64*1024*1024,
    max_age=3,
    busy_poll_us=200,
    busy_poll_disabled=False,
    pool_size=4096,
    enable_fec=True,
    disable_salvage_delivery=False,
    accumulator=AccumulatorConfig(),
)
```

| Field | Default | Description |
|-------|---------|-------------|
| `recv_buf` | `64 MB` | `SO_RCVBUF` size |
| `max_age` | `3` | Slot retention window |
| `busy_poll_us` | `200` | Linux `SO_BUSY_POLL` µs |
| `busy_poll_disabled` | `False` | Pass `True` to disable busy poll explicitly |
| `pool_size` | `4096` | Number of 2 KiB buffers in the zero-copy pool |
| `enable_fec` | `True` | Reed-Solomon recovery on dropped data shreds |
| `disable_salvage_delivery` | `False` | Drop salvaged tail txs for lowest p99 |
| `accumulator` | `AccumulatorConfig()` | FEC and stuck-batch tuning |

### `AccumulatorConfig`

| Field | Default | Description |
|-------|---------|-------------|
| `max_fec_sets_per_slot` | `32` | Per-slot FEC buffer cap |
| `stuck_batch_timeout_ms` | `50` | Force-finalize a stuck batch after this delay |

### Metrics

Read-only `@property` accessors on `ShredListener`:

| Group | Methods |
|-------|---------|
| **Throughput** | `data_shred_count_total`, `code_shred_count_total`, `bytes_received`, `slot_count` |
| **Decoder** | `batches_decoded_streaming_total`, `batches_decoded_fallback_total`, `batches_skipped_total`, `decode_errors_total` |
| **FEC** | `fec_recoveries_total`, `fec_recovery_failures_total`, `fec_sets_discarded_unused_total`, `fec_sets_evicted_early_total` |
| **Unparseable** | `unparseable_packets`, `unparseable_too_short`, `unparseable_variant`, `unparseable_payload`, `unparseable_slot_range` |
| **Slot lifecycle** | `slots_completed_total`, `slots_evicted_by_age`, `dropped_known_slots`, `harvested_batches_total`, `salvaged_tail_tx_total` |
| **Tail control** | `batches_force_finalized_corrupted_total`, `batches_force_finalized_timeout_total` |
| **Pool / I-O** | `pool_exhausted_count`, `last_io_error_kind`, `busy_poll_active` |

### Helpers

- `shredstream.classify_variant(byte: int) -> VariantKind | None` — Classify a shred variant byte
- `shredstream.pin_current_thread_to_cpu(cpu_id: int) -> None` — Best-effort thread pinning. Linux: `sched_setaffinity`; macOS: hint; other: no-op

## 🎯 Use Cases

ShredStream.com shred data powers a wide range of latency-sensitive strategies — HFT, MEV extraction, token sniping, copy trading, liquidation bots, on-chain analytics, and more.

### 💎 PumpFun Token Sniping

ShredStream.com SDK detects PumpFun token creations **~499ms before they appear on PumpFun's live feed** — tested across 25 consecutive detections:

<img src="https://raw.githubusercontent.com/shredstream/shredstream-sdk-python/main/assets/shredstream.com_sdk_vs_pumpfun_live_feed.gif" alt="ShredStream.com SDK vs PumpFun live feed — ~499ms advantage" width="600">

> Ready-to-run example included: see [`examples/pumpfun_creates.py`](examples/pumpfun_creates.py). Run with `python3 examples/pumpfun_creates.py [port]`.

## ⚙️ Configuration

### OS Tuning

```bash
# Linux -- increase max receive buffer
sudo sysctl -w net.core.rmem_max=67108864
sudo sysctl -w net.core.busy_read=200

# macOS
sudo sysctl -w kern.ipc.maxsockbuf=67108864
```

### Dependencies

- `solders>=0.21` — Required to decode the bincode-encoded transactions yielded by the iterator.

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
