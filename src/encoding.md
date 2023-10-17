# Structs Encoding Explanation

This document explains how various structs are encoded and decoded in the provided Rust code. The document covers the following structs:

1. [Message Struct](#1-message-struct)
2. [Header Struct](#2-header-struct)
3. [PeerEncodedInfo Struct](#3-peerencodedinfo-struct)
4. [NodePayload Struct](#4-nodepayload-struct)
5. [BroadcastPayload Struct](#5-broadcastpayload-struct)
6. [Marshallable Trait](#6-marshallable-trait)

---

## 1. Message Struct

**Purpose**: The `Message` struct represents various types of network messages, such as Ping, Pong, FindNodes, Nodes, and Broadcast. Each message has its header and associated payload.

**Encoding**:

| Field            | Length (bytes)  | Description                                     |
|------------------|-----------------|-------------------------------------------------|
| Message Type     | 1               | Type identifier for the message.                |
| Header           | Variable        | Header of the message.                          |
| Payload          | Variable        | Payload data specific to the message type.      |

- The length of the Header and Payload fields depends on the message type.

---

## 2. Header Struct

**Purpose**: The `Header` struct represents the header of a network message. It includes information such as the sender's binary ID, sender port, network ID, and reserved bytes.

**Encoding**:

| Field            | Length (bytes)  | Description                           |
|------------------|-----------------|---------------------------------------|
| Binary ID        | 32              | The binary ID of the sender.          |
| Nonce            | 8               | Nonce of the sender.                  |
| Sender Port      | 2               | Port of the sender (Little Endian).   |
| Network ID       | 1               | Network ID.                           |
| Reserved Bytes   | 2               | Reserved bytes.                       |

---

## 3. PeerEncodedInfo Struct

**Purpose**: The `PeerEncodedInfo` struct contains information about a peer, including their IP address, port, and binary ID.

**Encoding**:

| Field         | Length (bytes) | Description                       |
|---------------|----------------|-----------------------------------|
| IP Info       | Variable       | The IP address (IPv4 or IPv6).    |
| Port          | 2              | Port of the peer (LE encoding)    |
| Binary ID     | 32             | The binary ID of the peer.        |

- The length of the IP Info field depends on whether it's IPv4 or IPv6.

- IPv4 peers are identified with a leading `0` in the IP Info field. IPv6 peers do not have a leading `0`.

---

## 4. NodePayload Struct

**Purpose**: The `NodePayload` struct represents a collection of peer information.

**Encoding**:

| Field            | Length (bytes)  | Description                                     |
|------------------|-----------------|-------------------------------------------------|
| Number of Peers  | 2               | Number of peers in the payload.                 |
| Peer Info        | Variable        | Information about each peer (PeerEncodedInfo).  |

- The length of the Peer Info field depends on the number of peers in the payload.

- The number of peers is prepended as a 2-byte length before the Peer Info field.

---

## 5. BroadcastPayload Struct

**Purpose**: The `BroadcastPayload` struct represents the payload of a broadcast message. It includes the message's height and the message content.

**Encoding**:

| Field                  | Length (bytes) | Description                            |
|------------------------|----------------|----------------------------------------|
| Height                 | 1              | Height of the message.                 |
| Message Content Length | 4              | Length of the message (Little Endian). |
| Message Content        | Variable       | The content of the message.            |

- The length of the Message Content field depends on the size of the message.

- The length of the Message Content is prepended as a 4-byte length before the content.

---

## 6. Marshallable Trait

The `Marshallable` trait defines methods for encoding and decoding the structs into/from binary data.
