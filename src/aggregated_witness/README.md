## Aggregated Witness Communication Protocol

This module implements an aggregated witness–based reliable broadcast protocol for asynchronous, Byzantine-tolerant multi-threaded systems. It extends reliable and witness broadcast mechanisms by enabling hierarchical aggregation of reports.

---

### Overview

Each thread participates in a multi-round protocol consisting of:
1. **Value dissemination** - threads collect values, reports, and aggregated reports using round-scoped queues.
2. **Report Broadcast** — collected values are packaged into reports and reliably broadcast  
3. **Witness formation** - reports whose contents match known values are promoted to witnesses once the validity threshold is reached.
4. **Aggregated Report Broadcast** — upgraded witnesses are packaged into aggregated reports and reliably broadcast.
5. **Aggregated witness construction** - Aggregated reports whose component reports form a subset of known witnesses are upgraded to aggregated witnesses.
6. **Final delivery** - final values are delivered to application-facing channels upon reaching quorum thresholds.

Fault tolerance assumes

- `n` total threads  
- `t = ⌊(n − 1) / 3⌋` faulty threads  

with quorum thresholds defined as:

- **Validity threshold**: `n − t + 1`  
- **Agreement threshold**: `t + 1`

## Core Abstractions

### `AggregatedWitnessHub`

An object that initializes and manages per-thread communication infrastructure. It pre-allocates:

- Reliable broadcast signal channels,
- Witness and aggregated witness report channels,
- Thread-local message queues.

### `AggregatedWitnessCommunicator`

A representation of a single thread that encapsulates all communication primitives:

- **BasicChannels** for value delivery,
- **SignalChannels** for reliable broadcast (`Input`, `Echo`, `Vote`),
- **ReportChannels** for witness and aggregated witness dissemination,
- **Queues** for ordered, round-based message collection.

---

### Aggregated Reports

An Aggregated Report represents a higher-level witness formed by combining multiple compatible reports. Aggregated reports are dynamically upgraded to aggregated witnesses once their contents are validated against known witnesses.

---