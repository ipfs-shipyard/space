This is an overview of the current protocol design implemented in the `block-streamer/` application and a few open questions for the future.

## Current Design

The current design and implementation of the `block-streamer/` is intended to be a very simple way to transmit a file in IPFS across a radio link and reassemble using block data.

The file to be transmitted is read into 50 (tbd configurable) byte blocks. Each block consists of a CID, data, and links to other CIDs (if a root node). Each block is serialized into one binary blob, which is broken up into 40 byte (tbd configurable) chunks. Each chunk consists of a CID marker (first 4 bytes of CID), a chunk offset, and data. A header message consisting of the block CID is transmitted to the receiver first, followed by the chunks of the block's data+links, which are then reassembled in order. The current implementation is able to handle a dag with depth of two and can reassemble blocks sent out of order, but it can't yet handle chunks sent out of order.

*Current magic numbers and CID marker are placeholders to get things working, not final decisions.*

*Why not to use the CAR transport around blocks?*

In this initial implementation the CAR transport is not used. The reasoning was that this IPFS implementation should be designed for exchanging data over constrained communications links. This means it is likely that blocks will be transmitted individually, or even broken up into smaller chunks. There did not seem to be an immediate advantage to packaging these blocks up into a CAR, only to break that CAR up again into smaller chunks for transmission, when then blocks themselves could be transmitted as-is. However the CAR transport may still prove to be useful in this system in the future.

*Why decided to chunk blocks (hash+data) down to payload size)*

The [lab radio hardware](https://www.adafruit.com/product/3076) currently used in developing this system has a [strict payload size limit of 60 bytes](https://github.com/adafruit/RadioHead/blob/master/RH_RF69.h#L346-L347). While this radio may be more restrictive than typical satellite radios, it seems prudent to work under stricter requirements to ensure this system can scale both up and down to different payload limits. If sending individual 60-byte blocks the payload is already mostly consumed by the CID (36 bytes). This 60% overhead is not exactly efficient, so the decision was made to break blocks down into chunks which contain a CID marker (4 bytes), and a chunk offset (2 bytes), and a data blob, minimizing overhead to improve efficiency.

## Future Design Decisions

*Are there existing UDP data transfer protocols we can borrow from or use as-is?*

The current protocol for chunking/sending/assembling blocks was intentionally made simple to better understand the block transmission problem. It is very possible that an existing protocol built on UDP may provide the necessary chunking functionality, or at least functional pieces which can be built on.

Existing protocols which should be further investigated:
- [UDT](https://en.wikipedia.org/wiki/UDP-based_Data_Transfer_Protocol)
- [QUIC](https://www.chromium.org/quic/)
- [CoAP](https://en.wikipedia.org/wiki/Constrained_Application_Protocol)

*How should it handle specific data requests?*

A crucial part of this system will be correctly handling the transmission of a file across multiple communications passes, and dealing with lossy communication links, so the ability to request specific pieces of a DAG will be required. There are a number of different methods for specifying these pieces, such as by CID, with bitmasks, bloom filters, and sub-graphs. This decision will likely include a simple proof of concept implementing individual CID requests, followed by an analysis of the tradeoffs of other specification methods.

*Formal protocol messages*

The current implementation is a very simple one-way stream of block chunks. The future functional system will need to implement a formalized protocol with defined messages which allow for interactions such as requesting a specific CID or indicating that a CID has been received correctly. These will likely be created as required when implementing additional protocol functionality.