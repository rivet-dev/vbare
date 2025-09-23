## Comparison with Other Formats

Details not included in this evaluation:
- Number compression (e.g., static 64 bits vs using minimal bits)
- Zero-copy serialization/deserialization
- JSON support & extensions
- RPC

### Protobuf (versioned: yes)
- Poorly designed protocol in our opinion
- Makes migrations your problem at runtime by making everything optional
- Even worse, properties have default values (e.g., integers) which leads to subtle bugs with serious consequences
- Tracking field numbers in a file is tedious

### Cap'n Proto (versioned: yes)
- Includes the RPC layer as part of the library, which is outside the scope of what we want in our schema design
- Of the schema languages we evaluated, this provides by far the most flexible schema migrations
- Has poor language support — technically most major languages are supported, but the quality of the implementations is lacking. We suspect this is largely due to the complexity of Cap'n Proto itself compared to other protocols
- Generics are interesting, but we opt for simplicity with more repetition
- The learning curve seems the steepest of any other tool

### Cap'n Web (versioned: no)
- This is focused on RPC with JSON, which is not relevant to our needs

### CBOR/MessagePack/BSON (versioned: self-describing)
- Does not have a schema — it's completely self-describing
- Requires encoding the entire key, not suitable for our needs

### Flatbuffers (versioned: yes)
- Intended as a high-performance encoding similar to Protobuf
- Still uses indexes like Protobuf, unless you use structs
- To achieve what we wanted, we'd have to use only structs
- Schema evolution works similar to Protobuf
- Also requires writing field numbers in the file

### Bebop (versioned: no)
- Provides cross-platform compact self-contained binary encoding
- RPC is split out into a separate package, which we appreciate because we don't want to use someone else's RPC
- Includes JSON-over-Bebop which is nice — currently we rely on CBOR for this
- Could not find documentation on schema evolution
- We considered Bebop instead of BARE, but BARE seemed significantly simpler and more focused

### Borsh (versioned: no)
- Provides cross-platform compact self-contained binary encoding
- We considered Borsh instead of BARE, but BARE seemed significantly simpler and more focused

### Rust-specific Options (Postcard, etc.) (versioned: no)
- Also provides self-contained binary encoding
- Not cross-platform

