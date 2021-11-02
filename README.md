# Kadcast
Implementation of the Kadcast Network layer according to [the latest version of the paper to date (2021)](https://eprint.iacr.org/2021/996.pdf)

## Overlay Construction
Kadcast is an UDP-based peer-to-peer protocol in which peers form a structured overlay. 

> 💡 The specifications suggest that the protocol can be transport-agnostic, however this implementation uses the canonical choice of UDP.

All peers have an unique ID generated upon joining the network, which is used as an identifier unique to them.

> 💡 This implementation uses _128 bits identifiers_ generated from hashing the `<port>`|`<ip_octects>` concatenation truncated to the agreed 16 bytes. The hashing algorithm used is `Blake2`.

Following the [Kademlia's specifications](https://pdos.csail.mit.edu/~petar/papers/maymounkov-kademlia-lncs.pdf) for a structured network, the ID determines the peer position in a binary routing tree.

Each peer mantains the routing state in a set of `k-buckets` containing the location of the location of each peer. The location is fully determined by the tuple `(ip_addr, port, ID)`.

Each bucket comprises of a list of the `K` last recently seen peers, located at a given distance in relation to the peer. The distance is calculated as the `non-euclidean XOR metric` which represents: `d(x,y) = (x ^ y)`.

This implies that `i` buckets contain `K` Peers which distance `d` follow the formula: `2^i <= d < 2^(i +1)`.

> 💡 The factor `K`  is a system-wide parameter which determines the routing state and the lookup complexity.

- In case a bucket's peer list is full (i.e. `k` entries are already stored), new entries to that list may be inserted by dropping `LRU` (Least Recently Used) peers.

- Before dropping an entry from the list, the peer will check whether the peer to be dropped is still reachable, by sending a `PING` message and waiting for a `PONG` response. In case the `PONG` won't be collected within pre-determined timeout, the check fails and the peer is dropped from the list.

> 💡 By following a `LRU` policy, the protocol inherently is biased toward the older and more stable peers on the network.

###  Bootstrapping
Upon joining the Kadcast network, a peer retrieves from the so-called `bootstrap nodes` the locations of other neighbour peers, so to fill its buckets.

> 💡 From the specification: _When a peer first joins the network, it has to know the address of at least one bootstrapping node. It therefore sends `PING` messages to known peers to check whether they are actually online. Additionally, `PING` transmits the sending peer’s routing information to the recipient, thereby distributing its existence in the network._

The bootstrap node will reply with a `PONG` message which also contains its node info.

### Network discovery.
Starts just after the [bootstrapping] process.
The main goal of this phase is to fill up the `k-buckets` with `k` different peers.
- The process starts by sending a `FIND_NODE` message to the bootstraping nodes that have been added to the `k-bucket`s with the `PONG` messages received.
- Then the *lookup process* starts:
	1) The node looks up the 𝛼 closest peers regarding the XOR-metric in its own buckets.
	2) It queries this 𝛼 peers for the ID by sending `FIND_NODE` messages and wait for the `NODES` reply.
	3) The queried peers respond with a `NODES` reply containing the set of `k` peers they believe are the closest to `ID`.
	4) Based on the info acquired, the node builds a new set of closest peers and repeats steps *1* to *3* until it no longer gets peers closer to those it got from previous iterations.

> ⚠️ Note that like the bucket size, `𝛼` and `k` are global constants that will determine the redundancy and overhead of the network. The typical values are: `k = [20, 100]` & `𝛼 = 3`.

#### Periodic Network Manteinance
Each peer periodically refreshes every bucket with no activity. For each such bucket, it picks a random `ID` with appropriate distance, performs a look up, and populates its buckets with fresh routing information.

### Improving Broadcast Reliability And Performance
Instead of delegating the broadcast to one peer per bucket, we select `β` delegates. This is done to increase the probability of at least one honest peer to receive the message broadcasted. This is a measure intended to help with the inherently lossy nature of the UDP protocol without forcing the use of slow CRC checking procedures.

Since each broadcast process is repeated on every hop (decreasing the height), it is imperative that the peers ignore duplicate `CHUNK` messages. Moreover, transmission failures have to be considered. Because of these factors, this implementation includes the use of *forward error connection schemes* based on [RaptorQ](https://tools.ietf.org/pdf/rfc6330.pdf) specifications and [related rust library](https://github.com/cberner/raptorq).
The `FEC` overhead factor can be adjusted through the paramenter `f = (n-s)/s`.

As shown in the benchmarks, we can get full network coverage even assuming a 12% packet-loss ratio with a `β = 3` & `f = 0,15` (Please refer to the benchmarks included with the library documentation).

### Security
The specification provides solutions for DOS, Sybil and Eclipse attacks, as well as obstruction of block delivery. All advices have been taken into consideration during the development.

## Internal Architecture
For more information related to the internal architecture please check [the architecture diagram](ARCHITECTURE.md).
