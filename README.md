# Versioned Binary Application Record Encoding (VBARE)

_Simple schema evoluation with maximum performance_

VBARE is a tiny extension to [BARE](https://baremessages.org/) that provides a way of handling schema evoluation.

## Preface: What is BARE?

> BARE is a simple binary representation for structured application data. 
>
> - Messages are encoded in binary and compact in size. Messages do not contain
>   schema information — they are not self-describing.
>
> - BARE is optimized for small messages. It is not optimized for encoding
>   large amounts of data in a single message, or efficiently reading a message
>   with fields of a fixed size. However, all types are aligned to 8 bits,
>   which does exchange some space for simplicity.
>
> - BARE's approach to extensibility is conservative: messages encoded today
>   will be decodable tomorrow, and vice-versa. But extensibility is still
>   possible; implementations can choose to decode user-defined types at a
>   higher level and map them onto arbitrary data types.
>
> - The specification is likewise conservative. Simple implementations of
>   message decoders and encoders can be written inside of an afternoon.
>
> - An optional DSL is provided to document message schemas and provide a
>   source for code generation. However, if you prefer, you may also define
>   your schema using the type system already available in your programming
>   language.
> 
> [Source](https://baremessages.org/)

Also see the [IETF specification](https://www.ietf.org/archive/id/draft-devault-bare-11.html).

## Project goals

- fast -- self-contained binary encoding, akin to a tuple -> 
- simple -- can rewrite in under an hour
- portable -- cross-language & well standardized

non-goals:

- data compactness -> that's what gzip is for
- provide an rpc layer -> this is trivial to do yourself based on your specific requirements

## Use cases

- Defining network protocols
- Storing data at rest that needs to be able to be upgraded
    - Binary data in the database
    - File formats

## At a glance

- Every message has a version associated with it
    - either pre-negotiated (via something like an http request query parameter/handshake) or embedded int he message itself
- Applications provide functions to upgrade between protocol versions
- There is no evolution semantics in the schema itself, just copy and paste the schema to write the new one

## evolutino philosophy

- declare discrete versions with predefined version indexes
- manual evolutions simplify the application logic by putting complex defaults in your app code
- stop making big breaking v1 -> v2 changes, make much smaller changes with more flexibility
- reshaping structures is important -- not just changing types and names

## specification

### versions

each schema version is a monotomically incrementing <TODO: integer type>

### embedded version

embedded version works by inserting a <TODO: integer type> integer at the beginning of the buffer. this integer is used to define which version of the schema is being used.

the layout looks like this:

```
TODO
```

### pre-negotiated version

often times, you speicty the protocol version outside of the message iteself. for eaxmple, if making an http request with the version in the path like `POST /v3/users`, we can extract version 3 from the path. in this case, VBARE does not insert a version in to the buffer. for this, vbare simply acts as a simple step function for upgrading/downgrading version data structures.

## Implementations

- [TypeScript](./typescript/)
    - [Example Code](./typescript/examples/basic/src/migrator.ts)
- [Rust](./rust/)
    - [Example Code](./rust/examples/basic/src/lib.rs)

([Full list of BARE implementations](https://baremessages.org/))

_Adding an implementation takes less than an hour -- it's really that simple._

## Current users

- [Rivet Engine](https://github.com/rivet-dev/engine)
    - [Data at rest](https://github.com/rivet-dev/engine/tree/bbdf1c1c49e307ba252186aa4d75a9452d74fca7/sdks/schemas/data)
    - Internal network protocols ([tunnel](https://github.com/rivet-dev/engine/tree/bbdf1c1c49e307ba252186aa4d75a9452d74fca7/sdks/schemas/epoxy-protocol), [Epoxy](https://github.com/rivet-dev/engine/tree/bbdf1c1c49e307ba252186aa4d75a9452d74fca7/sdks/schemas/epoxy-protocol), [UPS](https://github.com/rivet-dev/engine/tree/bbdf1c1c49e307ba252186aa4d75a9452d74fca7/sdks/schemas/ups-protocol))
    - Public network protocol ([runner](https://github.com/rivet-dev/engine/tree/bbdf1c1c49e307ba252186aa4d75a9452d74fca7/sdks/schemas/runner-protocol))
- [RivetKit](https://github.com/rivet-dev/rivetkit)
    - [Client protocol](https://github.com/rivet-dev/rivetkit/tree/b81d9536ba7ccad4449639dd83a770eb7c353617/packages/rivetkit/schemas/client-protocol)
    - [Persisted state](https://github.com/rivet-dev/rivetkit/tree/b81d9536ba7ccad4449639dd83a770eb7c353617/packages/rivetkit/schemas/actor-persist)
    - [File system driver](https://github.com/rivet-dev/rivetkit/tree/b81d9536ba7ccad4449639dd83a770eb7c353617/packages/rivetkit/schemas/file-system-driver)

## Embedded vs Negotiated Version

TODO

## Clients vs Servers

- Only servers need to ahve the evolutions steps
- clients just send their version

## Downsides

- extensive migration code
- the older the version the more migration steps (though these migration steps should be effectively free)
- migration steps are not portable across langauges, but only the server needs to the migration step. so usually this is only done once.

## Comparison

- Protobuf (versioned: yes)
    - unbelievably poorly designed protocol
    - makes migrations your problem at runtime by making everything optional
    - even worse, makes properties have a default value (ie integers) which leads to subtle bugs with serious concequenses
    - tracking field numbers in a file is a pain in the ass
- Cap'n'proto (versioned: yes)
    - includes the rpc layer as part of the library, this is out of the scope of what we want in our schema design
    - of the schema languages we evaluated, this provides by far the most flexible schema migrations
    - has poor language support. technically most major languages are supported, but the qulaity of the ipmlementations are lacking. i suspect this is largely due to the complexity of capnproto itself compared to other protocols.
    - generics are cool. but we opt for simplicity with more repetition.
    - the learning curve seems the steepest of any other tool
- cap'n'web (versioned: no)
    - this is focused on rpc with json. not relevant to what we needed.
- cbor/messagepack/that mongodb one (versioned: self-describing)
    - does not have a schema, it's completley self-describing
    - requires encoding the entire key, not suitable for our needs
- Flatbuffers (versioned: yes)
    - intented as a high performance encoding similar to protobuf
    - still uses indexes like protobuf, unless you use structs
    - to achieve what we wanted, we'd have to use just structs
    - schema evolution works similar to protobuf
    - also requires writing field numbers in the file
- https://crates.io/crates/bebop (verisoned: no)
    - provides cross platform compact self-contained binary encoding
    - rpc is split out in to a separate package, which i like because i don't want to use someone else's rpc
    - includes json-over-bebop which is nice. currenlty we rely on cbor for this.
    - could not find docs on schema evolution
    - considered bebop instead of bare, but bare seemed significantly simpler and more focused
- https://crates.io/crates/borsh (versioned: no)
    - provies cross platform compact self-contained binary encoding
    - considered borsh instead of bare, but bare seemed significantly simpler and more focused
- rust options like postcard/etc (versioned: no)
    - also provides self-contained binary encoding
    - not cross platform 

other deatils not included in this evaluation:
- number compression (ie static 64 bits vs using minimal bits)
- zero-copy ser/de
- json support & extensions
- rpc

## FAQ

### Why is copying the entire schema for every version better than using decorators for gradual migrations?

- decorators are limited and get very complicated
- it's unclear what version of the protocol a decorator takes effect -- this is helpful
- generated sdks become more and more bloated with every change
- you need a validation build step for your validators
- things you can do with manual migrations

### Why not include RPC?

RPC interfaces are trivial to implement yourself. Libraries that provide RPC interfaces tend to add extra bloat & cognitive load over things like abstracting transports, compatibility with the language's async runtime, and complex codegen to implement handlers.

Usually, you just want a `ToServer` and `ToClient` union that looks like this: [ToClient example](https://github.com/rivet-dev/rivetkit/blob/b81d9536ba7ccad4449639dd83a770eb7c353617/packages/rivetkit/schemas/client-protocol/v1.bare#L34), [ToServer example](https://github.com/rivet-dev/rivetkit/blob/b81d9536ba7ccad4449639dd83a770eb7c353617/packages/rivetkit/schemas/client-protocol/v1.bare#L56)


### Isn't copying the schema going to result in a lot of duplicate code?

- yes. after enough pain and suffering of running production APIS, this is what you will end up doing manually, but in a much more painful way.
- having schema versions also makes it much easier to reason about how clients are connecting to your system/the state of an application. incremental migrations dno't let you consider other properties/structures.
- this also lets you reshape your structures.

### Don't migration steps get repetitive?

- most of the time, structures will match exactly. most languages can provide a 1:1 migration.
- the most complicated migration steps will be very deeply nested structures that changed, but that's pretty simple

## License

MIT

