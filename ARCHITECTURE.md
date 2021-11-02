# Internal Architecture

![architecture](architecture.jpg)

## kadcast::Peer
Your entry point

## Message Handler
The core of the library. It handles the incoming messages and replies accordly to them

## Mantainer
Performs the initial bootstrap and starts the k-table maintenance. 
Keep track of idle buckets and it's in charge of trigger the routing nodes lookup.

## Wire Network
It is responsible to instantiate the UDP server for receive the messages from the network. 
It's in charge of outgoing communication too (UDP client).

It performes message serialization/deserialization. If needed by the message type it applies the FEC encoding/decoding

### RaptorQ Encoder
Splits a single broadcast message in multiple chunks applying the RaptorQ FEC algorithm

### RaptorQ Decoder
Joins multiple encoded chunk in a single decoded broadcast message. It own a cache to avoid processing brodcasted identical messages multiple times

## Channels
### Inbound Channel
Message passing between incoming UDP packets (deserialized by the WireNetwork) and Message Handler
### Outbound Channel
Channel dedicated to any message which need to be transmitted to the network.
### Notification Channel
Once a broadcast message has been successfully received, it's put on this channel to decouple the notificated thread execution to the rest of the library threads