## Quorum-Based Reliable Broadcast Protocol

This module implements a quorum-based reliable broadcast protocol**, extending broadcast communication with an Input–Echo–Vote signaling framework to ensure consistent message delivery despite faulty or unresponsive threads. The implementation is designed to support multi-instance, multi-round execution, serving as a foundational layer for higher-order protocols.

---

## Overview

The reliable broadcast protocol proceeds through three phases:

1. **Input** — the sender initiates a broadcast
2. **Echo** — receivers confirm receipt
3. **Vote** — receivers commit once quorum thresholds are met

Each thread runs a background task responsible for listening for incoming signals, tracking per-instance state and counters, and applying threshold-based transitions.

Thresholds are defined as 

- **Validity threshold**: `n − t`
- **Agreement threshold**: `t + 1`

where,

- `n` be the number of threads
- `t = ⌊(n − 1) / 3⌋` be the maximum number of faulty threads

---

## Core Abstractions

### `ReliableCommunication` Trait

The trait extends the `BasicCommunication` interface with reliable broadcast semantics:

- `reliable_broadcast` — initiates a reliable broadcast for a given instance and round
- `reliable_recv` — retrieves a reliably delivered message from the local queues
- `initialize_reliable_handle` — spawns a background task that processes protocol signals
- `terminate_reliable_handle` — aborts the background protocol task

---

### `ReliableHub`

An object responsible for initializing multiple reliable communicators:

- Distributes basic message receivers
- Creates dedicated signal channels for protocol coordination
- Hands out `ReliableCommunicator` instances to simulated threads

---

### `ReliableCommunicator`

A representation of a single thread that implements both `BasicCommunication` and `ReliableCommunication`, participating in reliable broadcast. Each communicator maintains:

- `MessageChannels` for delivering finalized messages
- `SignalChannels` for exchanging protocol signals
- `BasicQueues` for inbound message buffering
- A background task that executes protocol logic

---

### `Signal`

A protocol-level message exchanged between threads:

- Signal type (`Input`, `Echo`, or `Vote`)
- Encapsulated content (message or report)
- Consensus instance number
- Round number