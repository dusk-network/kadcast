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

See [`RaptorQDecoderConf`](raptorq/decoder.rs) for cache limits.

Please note that the actual RaptorQ encoding and decoding processes are carried out using the RaptorQ library and are not detailed here. The focus is on how the Rust code manages the chunked broadcast payloads in the context of RaptorQ encoding.