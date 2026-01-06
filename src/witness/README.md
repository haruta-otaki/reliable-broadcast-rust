## Witness-Based Reliable Broadcast Protocol

This module implements a witness-based reliable broadcast protocol, extending quorum-based reliable broadcast with local validation and witness formation. Threads collect values, exchange reports via reliable broadcast, and convert consistent reports into witnesses once quorum conditions are met. 

---

## Overview

The witness-based protocol executes in rounds, building on reliable broadcast:

1. **Value Collection** — threads collect values broadcast by peers  
2. **Report Broadcast** — collected values are packaged into reports and reliably broadcast  
3. **Witness Formation** — reports whose values are locally consistent are converted into witnesses  
4. **Witness Delivery** — once a quorum of witnesses is observed, the values are delivered  

Each thread runs a background task that listens for incoming values, reports, and protocol signals. Per-round state is tracked independently, allowing multiple rounds to concurrently execute.

Fault tolerance assumes

- `n` total threads  
- `t = ⌊(n − 1) / 3⌋` faulty threads  

with quorum thresholds defined as:

- **Validity threshold**: `n − t + 1`  
- **Agreement threshold**: `t + 1`

---

## Core Abstractions

### `WitnessCommunication` Trait

The trait defines the behavior required for a thread to participate in witness-based reliable broadcast. The trait extends `ReliableCommunication` and provides:

- `witness_broadcast` — broadcasts an initial value for a witness round  
- `witness_collect` — collects validated witness values for a round  
- `initialize_witness_handle` — spawns the background witness-processing task  
- `terminate_witness_handle` — aborts the witness task  

---

### `WitnessHub`

An object responsible for initializing and wiring multiple witness communicators:

- Distributes basic message receivers  
- Creates dedicated channels for reliable and witness protocol tasks  
- Hands out `WitnessCommunicator` instances to simulated threads  

---

### `WitnessCommunicator`

A representation of a single thread participating in witness-based reliable broadcast:

- `MessageChannels` for standard message delivery  
- `SignalChannels` for reliable broadcast coordination  
- `ReportChannels` for delivering reports and witnesses  
- `BasicQueues` for inbound message buffering  
- Background tasks for both reliable and witness protocols  

---

### `Report`

A `Report` represents a collection of values produced during a witness round.

Two report types are supported:
- `Report` — an unvalidated collection of values  
- `Witness` — a validated report whose values satisfy local consistency conditions  