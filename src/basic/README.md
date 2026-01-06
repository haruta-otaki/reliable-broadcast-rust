## Basic Message-Passing Protocol

This module implements a foundational asynchronous message-passing layer. It provides point-to-point messaging, broadcast primitives, and structured message buffering, serving as the lowest-level building block for higher-order protocols: Quorum-Based Reliable Broadcast, Witness-Verified Broadcast, and Aggregated Witness Broadcast.

---

## Overview

The basic communication layer models a system of asynchronous threads communicating over Tokio channels. 
Each node maintains:
- A shared set of outbound channels to all peers  
- A local inbound receiver  
- Per-sender message queues that filters with respect to protocol and round 

Messages are serialized using **JSON**, enabling protocol-agnostic transport and uniform handling across different broadcast abstractions.

---

## Core Components

### `BasicCommunication` Trait

A generic interface for a thread to participate in message passing any amount of times with different payloads:

- `basic_send` — send a message to a specific node  
- `basic_broadcast` — broadcast a message to all nodes  
- `basic_recv` — receive the next matching message from local queues  

---

### `BasicHub`

An object that spawns multiple communicators:

- Initializes per-thread receivers  
- Shares cloned transmitters across all nodes  
- Distributes `BasicCommunicator` instances to simulated threads  

---

### `BasicCommunicator`

A representation of a single thread in the system that implements the `BasicCommunication` trait and combines:

- `MessageChannels` for outbound communication  
- `BasicQueues` for inbound buffering and filtering  
- A unique thread identifier  

---

### `MessageChannels`

Manages outbound communication:

- Sends messages to specific nodes  
- Broadcasts messages to all nodes  
- Serializes messages to JSON prior to transmission  

---

### `BasicQueues`

Manages inbound message buffering:

- Maintains per-sender queues  
- Filters messages by protocol name, instance number, and round  
- Supports blocking asynchronous receives until a matching message arrives  

---

### `Message`

A strongly-typed message container carrying:

- Protocol identifier  
- Sender ID  
- Payload  
- Optional instance number  
- Round number  
- Optional dimension metadata (for higher-level algorithms)  