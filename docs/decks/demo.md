<!--
theme: default
headingDivider: 3
-->

## File -> IPFS DAG

<script type="module">
  import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.esm.min.mjs';
  mermaid.initialize({ startOnLoad: true });
</script>

<div class="mermaid">
flowchart TD
    FC("File Content = '0123456789'")
    CZ["File chunking size = 2B"]
    FC --> CZ
    subgraph Chunking
        A["'01'"]
        B["'23'"]
        C["'45'"]
        D["'67'"]
        E["'89'"]
        CZ --> A
        CZ --> B
        CZ --> C
        CZ --> D
        CZ --> E
    end
    subgraph Hashing Chunks
        AA["'01'"] --> AH["bafkreietrw4mt6bmrs2y2pz66t6skabwusgsnjysou6s7xs2xub2qxfl6q"]
        EP["..."]
        EE["'89'"] --> EH["bafkreignoc7kai7xkkqfmsv3n3ii2qwbiqhs4m7ctekokxql4fmv4jhuli"]
        A --> AA
        B --> EP
        C --> EP
        D --> EP
        E --> EE 
    end
    subgraph Form Stem Node
        LT("bafkreietrw4mt6bmrs2y2pz66t6skabwusgsnjysou6s7xs2xub2qxfl6q
        bafkreictl6rq27rf3wfet4ktm54xgtwifbqqruiv3jielv37hnaylwhxsa
        bafkreiebc6dk2gxhjlp52ig5anzkxkxlyysg4nb25pib3if7ytacx4aqnq
        bafkreicj2gaoz5lbgkazk4n7hhm3pm2ckivcvrwshqkbruztqji37zdjza
        bafkreignoc7kai7xkkqfmsv3n3ii2qwbiqhs4m7ctekokxql4fmv4jhuli")
        AH --> LT
        EH --> LT
    end
    subgraph Hash Stem
        RT["Root = bafybeicbshh2atg556w77jzb5yl4e63fefisnutf32l7byzrteosqjhb6i"]
        LT-->RT
    end
</div>

## Networking Overview

<div class="mermaid">
flowchart TD
    subgraph Vehicle
        A["Application (e.g. Watcher)"] -- ApplicationAPI/UDP --> B[Myceli]
        B <-- CommsAPI/UDP --> C[Comms]
    end
    subgraph Radio
        Z[Data Transfer Protocol]
    end
    subgraph Ground
        F["Service (e.g. Controller)"] -- ApplicationAPI/UDP --> E[Myceli]
        E <-- CommsAPI/UDP --> G[Comms]
    end
    C <--> Z
    G <--> Z
</div>

## Shipper Protocol

<div class="mermaid">
sequenceDiagram
    participant O as Operator
    participant G as Ground IPFS
    participant S as Space IPFS
    Note over O,G: Operator commands IPFS <br/> node to transmit a file
    O->>G: TransmitFile(path)
    Note over G,S: Transfer of blocks <br/> 1. File is chunked into blocks, each with a CID <br/> 2. Root block contains links to child CIDs <br/> 3. Blocks are transmitted over UDP-radio 
    loop Until DAG is Complete
        G->>S: GetMissingDagBlocks(CID): [Block] <br/> 
        Note over G,S: If blocks are missing, ground retransmits
        G->>S: While blocks remain missing, <br/>TransmitBlock(CID)
    end
</div>

## Sync Protocol

<div class="mermaid">
sequenceDiagram
    participant G as Ground
    participant V as Vehicle
    Note over G: Import File
    Note left of G: Available CIDs: <br /> bafybeicbshh2atg556w77jzb5yl4e63fefisnutf32l7byzrteosqjhb6i (file.name) <br /> bafkreietrw4mt6bmrs2y2pz66t6skabwusgsnjysou6s7xs2xub2qxfl6q <br/> ... 4 more (5 leaves in total)
    loop Until synchronized
        G ->> V: "Push" Send CIDs to Expect (& File Name)
        Note right of V: Available CIDs: <br />(none)<br/> Missing CIDs: <br /> (all 6)
        G ->> V: Send Block
        Note over V: Hash, store.
        Note right of V: Available CIDs: <br /> bafkreignoc7kai7xkkqfmsv3n3ii2qwbiqhs4m7ctekokxql4fmv4jhuli <br /> Missing CIDs: <br /> ... 5 CIDs remain ...
        G --X V: Attempt to send blocks, packets dropped 
        V ->> G: "Pull" Send CIDs for blocks to send/re-send
        G ->> V: Send Block (bafybeicbshh2atg556w77jzb5yl4e63fefisnutf32l7byzrteosqjhb6i)
        Note over V: Hash, store.
    end
</div>

## Sync Protocol - Special Failure

<div class="mermaid">
sequenceDiagram
    participant G as Ground
    participant V as Vehicle
    Note over G: Import File
    Note left of G: Available CIDs: <br /> bafybeicbshh2atg556w77jzb5yl4e63fefisnutf32l7byzrteosqjhb6i (file.name) <br /> bafkreietrw4mt6bmrs2y2pz66t6skabwusgsnjysou6s7xs2xub2qxfl6q <br/> ... 4 more (5 leaves in total)
    G --X V: "Push" Send CIDs to Expect (& File Name)
    Note right of V: Available CIDs: <br />(none)<br/> Missing CIDs: <br /> (none - the push never got here)
    G ->> V: Send Block (bafybeicbshh2atg556w77jzb5yl4e63fefisnutf32l7byzrteosqjhb6i)
    Note over V: Hash, store.
    Note over V: Parse as stem, passes - has 5 children. 
    loop For each child CID
        Note over V: Neither available nor marked as missing, mark as missing.
    end
    Note right of V: Available CIDs: <br /> bafybeicbshh2atg556w77jzb5yl4e63fefisnutf32l7byzrteosqjhb6i (file.name) <br /> bafkreignoc7kai7xkkqfmsv3n3ii2qwbiqhs4m7ctekokxql4fmv4jhuli <br /><br /> Missing CIDs: <br /> ... 4 CIDs remain ...
    V ->> G: Pull (5 CIDs mentioned in stem)
    loop Other CIDs in pull
        G ->> V: Send Blocks
    End
</div>

## Anatomy of a network message

### SCALE encoding

* Compact, Binary
* Fixed-length fields: generally just the bytes
* Variable length (strings, lists): length-prefixed
* VarInt: one doesn't need 8 bytes to express the length of most lists
* Union/Variant: Leading VarInt (typically 1 byte) to distinguish

### Myceli message chunk

| 1 byte     |
|------------|
| Chunk Type |

- 0 = Leading = more chunks after this needed to piece the message together
- 1 = Final = last chunk in message.
- 2 = Single = Special case, whoe message fits in a configured MTU

### Myceli message chunk

| 1 byte     | Variable (Likely 1-2 bytes)   | N (previous field) bytes                                       |
|------------|-------------------------------|----------------------------------------------------------------|
| 2 = Single | Length of the bytes to follow | Data: deserialize this payload as a Message (see next section) |

- Special case when the whole message can fit this way
- Avoids message ID & sequence ID overhead

### Myceli message chunk

| 1 byte              | Variable (1-4 bytes)          |
|---------------------|-------------------------------|
| 1/2 = Leading/Final | Integer Message ID; max=65535 |

- Keep all chunks with the same Message ID together

### Myceli message chunk

| 1 byte              | Variable (1-4 bytes)          | Variable (1-6 bytes) |
|---------------------|-------------------------------|----------------------|
| 1/2 = Leading/Final | Integer Message ID; max=65535 | Sequence Number      |

- If the Chunk Type is Final, then Sequence Number = N
- For a given Message ID, need chunks with sequence #'s 0, 1, 2, ... N

### Myceli message chunk

| 1 byte              | (1-4 bytes)                   | (1-6 bytes)     | (1-6 bytes) | N Bytes |
|---------------------|-------------------------------|-----------------|-------------|---------|
| 1/2 = Leading/Final | Integer Message ID; max=65535 | Sequence Number | Length      | Data    |

Once all (0..Final) sequence numbered chunks of a message are received

- Concatenate Data fields in sequence # order
- Deserialize the result as a Message Container...

### Message Container

| 8 bytes                       | 1-6 bytes      | N bytes |
|-------------------------------|----------------|---------|
| Hash of data in message chunk | length of data | data    |

- Verify the hash of data. Ask for re-send if it doesn't match.
- Deserialize data as a Message...

### Myceli messages

| 1 byte       |
|--------------|
| Message Type |

Determines which fields follow

- 0 = Shipper protocol (explicit transmitting between Mycelis)
- 1 = Application API (communicating with external services, e.g. controller)
- 2 = Error Message
- 3 = Sync protocol (sharing data between Mycelis)

### Myceli messages

| 1 byte  | 1 byte           |
|---------|------------------|
| 1 = API | Specific Message |

Determines which fields follow

- 0 = ImportFile (request)
- 1 = FileImported (response)
- 10 = TransmitDag (request)
- ...
- 27 = Acknowledge (general response)

### Myceli messages

Request Myceli send a DAG to another Myceli

| 1 byte  | 1 byte           | 1 byte | N bytes | 1 byte | N bytes | 1 byte  |
|---------|------------------|--------|---------|--------|---------|---------|
| 1 = API | 10 = TransmitDag | Length | CID     | Length | Target  | Retries |

- CID = root CID of DAG
- target = UDP endpoint to send to
- Retries = Number of automatic retries before giving up (max 255)

## Some Optimizations

* Reduced Block Size
* Implementing on necessary CID/block types
* Separate build for smaller binary

### Myceli Mini

* Fewer dependencies
    - Logger that does not involve regex
    - Storage that does not involve SQL
* Without any debugging info
* Stripped of Symbols
* Compiler optimization turned to size, not speed
* Assertions & overflow checks off
* Artifact compressed to highest level

Largest version: 128 MB (debug, local dev build)  
Smallest: 562 KB (arm small build, xz compression)