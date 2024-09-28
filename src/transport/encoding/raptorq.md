## ChunkedPayload Struct

**Purpose**: The `ChunkedPayload` struct is used for encoding and decoding broadcast payloads using RaptorQ encoding. It represents a single chunk of a larger broadcast message.

**Encoding**:

- The encoded `ChunkedPayload` is constructed from a `BroadcastPayload`.
- The encoding of a `ChunkedPayload` consists of the following components:
  - `RAY_ID` (Unique Identifier): A 32-byte hash of the broadcast payload, excluding the height field.
  - `ObjectTransmissionInformation` (RaptorQ header): A 12-byte header specific to the RaptorQ encoding scheme.
  - `Encoded Chunk`: The chunked and encoded data using RaptorQ.

| Field               | Length (bytes) | Description                       |
|---------------------|----------------|-----------------------------------|
| RAY_ID (Blake2s256) | 32             | Unique identifier for the chunked payload. |
| Transmission Info   | 12             | Object Transmission Information (RaptorQ header). |
| Encoded Chunk       | Variable       | The RaptorQ encoded chunk of the payload. |

**Decoding**:

- When a `BroadcastPayload` is transformed into a `ChunkedPayload`, it checks if the payload length is at least `MIN_CHUNKED_SIZE`, which is the minimum required length to consider it a valid `ChunkedPayload`. If not, an error is raised.

- The `ChunkedPayload` holds the following components:
  - `RAY_ID`: The unique identifier for the chunk, extracted from the first 32 bytes of the `gossip_frame`.
  - `ObjectTransmissionInformation`: The 12-byte RaptorQ header, parsed from the `gossip_frame` bytes.
  - `Encoded Chunk`: The remaining bytes after RAY_ID and RaptorQ header, containing the encoded chunk.

- `ChunkedPayload` is used in a cache to manage the decoding process for broadcast messages. It tracks the state of a broadcast message's chunk as either receiving or processed.

- The cache stores the `RAY_ID`+`ObjectTransmissionInformation` (aka `ChunkedHeader`) of the broadcast message as the key and the `CacheStatus` as the value, which tracks the state.

- The `CacheStatus` can be in two states:
  1. **Receiving**: In this state, a RaptorQ decoder is initialized with the `ObjectTransmissionInformation`. The decoder processes incoming encoded chunks and attempts to decode them. If a chunk is successfully decoded, the message is checked for integrity, and if it's valid, it's stored as a fully processed message. If not, it's discarded.
  2. **Processed**: In this state, the message has been successfully decoded and processed. The `CacheStatus` holds a timestamp for when it was last processed, and if the message is expired based on the cache's TTL, it can be removed from the cache.

- The cache is pruned periodically to remove expired messages.

Please note that the actual RaptorQ encoding and decoding processes are carried out using the RaptorQ library and are not detailed here. The focus is on how the Rust code manages the chunked broadcast payloads in the context of RaptorQ encoding.