# Multi-Threaded Message-Passing Infrastructure in Rust 

This project is a high-performance, lock-free, multi-threaded message-passing framework in Rust implementing layered reliable broadcast protocols with quorum-based fault tolerance. The infrastructure was developed as part of an undergraduate research effort at Davidson College under the guidance of **Dr. Hammurabi Mendes**. The available protocols are all implemented and tested for correctness in multi-threaded environments with concurrent failure scenarios.

---

## Overview

This framework provides an infrastructure for sending, receiving, and broadcasting messages across multiple threads. Each protocol adds guarantees through layered verification while preserving modularity, extensibility, and performance:

1. Basic Message-Passing Protocol
2. Quorum-Based Reliable Broadcast
3. Witness-Verified Broadcast
4. Aggregated Witness Broadcast 

---

## Repository Structure

```text
src/
├── basic/              # Basic message-passing layer
├── reliable/           # Quorum-based reliable broadcast
├── witness/            # Witness-verified broadcast
├── aggregated_witness/ # Aggregated witness broadcast
├── json/               # Message serialization utilities
├── lib.rs              # Shared interfaces and exports
└── main.rs             # Reference entry point
```

## Protocols

### 1. Basic Message-Passing Layer
The Basic Message-Passing layer defines the foundational interface for  asynchronous, multi-round communication between threads. Higher-level protocols may be built on top of this layer such that the low-level coordination mechanisms (e.g. channel management, buffering, and message routing) may be abstracted away.

This layer provides three core primitives:
- `send` — asynchronously transmit a message to a specific thread by identifier
- `recv` — retrieve the next matching message from a thread’s local receive queues
- `broadcast` — asynchronously deliver a message to all threads in the system

---

### 2. Quorum-Based Reliable Broadcast

The Quorum-Based Reliable Broadcast protocol extends the basic broadcast primitive with quorum-driven acknowledgment and agreement to ensure consistent message delivery in the presence of thread failures. 

Each broadcast is executed as a multi-phase signaling protocol consisting of three stages: 
- `Input` — the initiation of the broadcast carrying the message and protocol metadata
- `Echo` — the rebroadcast of valid Input signals to acknowledge receipt.
- `Vote` — a signal indicating agreement emitted once a quorum of Echo signals is observed.

Each thread performs their individual phase transitions after confirmation from a quorum of `n - t` threads is observed, where: 
- **n**: total number of threads 
- **t**: maximum number of tolerated thread failures 

**Protocol guarantees:**
- `Validity` — if a correct thread broadcasts a message, all correct threads eventually deliver it.
- `Agreement` — no two correct threads deliver different messages for the same instance.
- `Fault tolerance` — delivery is preserved despite up to t faulty or unresponsive threads.

---

### 3. Witness-Verified Broadcast

Witness-Verified Broadcast augments Quorum-Based Reliable Broadcast by introducing witness reports. Witness reports provide a locally verifiable evidence that a broadcast value is consistent with previously received messages. This protocol adds a second verification layer rather than trusting quorum agreement alone, by requiring that reports be locally validated before contributing to delivery.

Witness-Verified Broadcast proceeds in rounds, each tracked independently by every thread. 

1. `Value Collection` - every thread continuously receives messages for the current round.

2. `Report (Reliable) Broadcast` - a thread constructs a Report containing its collected messages once it observes a quorum of values, and reliably broadcasts it using the underlying Quorum-Based Reliable Broadcast protocol.

3. `Witness Verification` - a thread, upon receiving a Report, checks whether the messages contained in the report form a subset of its locally received values, verifying the report as a Witness if the condition holds. 

4. `Witness Quorum & Delivery` -  a thread delivers the round’s value set once it collects a quorum of validated witnesses.

**Protocol guarantees:**
- `Validity` — if a correct thread reports a value, all correct threads eventually observe a witness containing that value.
- `Agreement` — for any two correct threads that deliver in the same round, there exists a quorum of witnesses whose reported values are mutually consistent via subset inclusion.
- `Integrity` — only values that were actually received by correct threads may be delivered.
- `Fault tolerance` — delivery is preserved despite up to t faulty or unresponsive threads.

---

### 4. Aggregated Witness Broadcast

Aggregated Witness Broadcast Protocol coordinates multi-round, multi-layer witness verification, using aggregated reports. Aggregated reports sematically the same as witness reports, containing previously validated Witnesses. 

**Protocol Gurantees:**
- `Validity` — if a correct thread validates a witness, then all correct threads eventually observe an aggregated witness that includes that witness.
- `Agreement` — for any two correct threads that deliver in the same round, there exists a quorum of aggregated witnesses whose underlying witness sets are mutually consistent via subset inclusion.
- `Integrity` — only witnesses that were themselves validated by correct threads may appear in aggregated witnesses, and only values that were originally received may be delivered.
- `Fault tolerance` — delivery is preserved despite up to t faulty or unresponsive threads.

---

## Future Work

Future work includes implementing the barycentric agreement protocol in addition to broadcast protocols; specifically, a variant of the textbook barycentric agreement algorithm is being implemented through alternative formulations grounded in combinatorial topology. 

---

## Usage

The framework is a simple, high-level API that allows developers to send, receive, and broadcast messages, execute multi-round protocols, and swap or extend broadcast mechanisms without modifying core logic.

The best way to get started is to follow the main.rs file, which serves as  reference implementation. The file demonstrates how to import and compose the provided communication modules, spawn asynchronous threads, and execute different protocols by selecting the desired mode at runtime. Detailed standalone examples will be added in the future.
