# Architecture Layout

This document explains the architecture of the library components. The low-level networking and threading infrastructure makes use of async [Tokio runtime](https://docs.rs/tokio). In the future, the code will be made modular so to allow the use of [different runtimes](https://rust-lang.github.io/async-book/08_ecosystem/00_chapter.html).

The architecture follows the diagram below. For usage examples please refer to the [crate's documentation](https://crates.io/crates/kadcast)

![architecture](architecture.jpg)

## Kadcast Peer
The `kadcast::peer` is the entry point of the library and the only public API. It implements the Kadcast Network Peer.

## Message Handler
The `Message Handler` represents the core component of the library. It handles the incoming messages and replies according to the specification.

## Maintainer
The `Maintainer` performs the [initial bootstrap](../bootstrapping) as well as the `k-table` [maintenance](../periodic-network-manteinance).
It keeps track of idle buckets and is in charge of triggering the routing nodes lookup.

## Wire Network
It is responsible to instantiate the UDP server that receives the messages from the network. It's also in charge of outgoing communication (UDP client).
It performs message serialization/deserialization and applies the FEC encoding/decoding where appropriate.

### RaptorQ Encoder
The `RaptorQ Encoder` splits a single broadcast message in multiple chunks according to the RaptorQ FEC algorithm.

### RaptorQ Decoder
The `RaptorQ Encoder` joins the encoded chunks in a single decoded broadcast message. It keeps a cache to avoid processing brodcasted identical messages multiple times.

## Channels
### Inbound Channel
Provides message passing between incoming UDP packets (deserialized by the WireNetwork) and Message Handler
### Outbound Channel
Channel dedicated to any message which needs to be transmitted to the network.
### Notification Channel
Successfully received broadcast messages are dispatched to the `Notifier` through the `Notification Channel`. This is done in order to safely forward messages to the user's callback without any risk for the latter to block any other thread.
