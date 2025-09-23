# Versioned Binary Application Record Encoding (VBARE)

_Simple schema evolution with maximum performance_

VBARE is a tiny extension to [BARE](https://baremessages.org/) that provides a way of handling schema evolution.

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

## Project Goals

**Goals:**

- **Fast** — Self-contained binary encoding, similar to a tuple structure
- **Simple** — Can be reimplemented in under an hour
- **Portable** — Cross-language support with well-defined standardization

**Non-goals:**

- **Data compactness** — That's what gzip is for
- **RPC layer** — This is trivial to implement yourself based on your specific requirements

## Use Cases

- **Network protocols** — Allow the server to cleanly evolve the protocol version without breaking old clients
- **Data at rest** — Upgrade your file format without breaking old files

## Overview

VBARE works by declaring a schema file for every possible version of the schema, then writing conversion functions between each version of the schema.

**Versions**

Each message has an associated version, an unsigned 16 bit integer. Each version of the protocol increases monotonically from 1 (e.g. `v1`, `v2`, `v3`). This version is specified in the file name (e.g. `my-schema/v1.bare`).

**Schema Evolution & Converters**

On your server, you manually define code that will convert between versions for both directions (upgrade for deserialization, downgrade for serialization).

There are no evolution semantics in the schema itself — simply copy and paste from `v1` to `v2` the schema to write a new version.

**Servers vs Clients**

Servers need to include converters between all versions.

Clients only need to inlucde a single version of the schema since the server is responsible for version conversion no matter what version you connect with.

**Embedded vs Negotiated Versions**

Every message has a version associated with it. This version is either:

- Pre-negotiated (via mechanisms like HTTP request query parameters or handshakes)
    - For example, you can extract the version from a request like `POST /v3/users`
- Embedded in the message itself in the first 2 bytes of the message (see below)

**Embedded Binary Format**

Embedded version works by inserting an unsigned 16 bit integer at the beginning of the buffer. This integer is used to define which version of the schema is being used.

The layout looks like this:

```
+-------------------+-------------------+
|  Schema Version   |    BARE Payload   |
|   (uint16, 2B)    |   (variable N B)  |
+-------------------+-------------------+
```

## Philosophy

The core of why VBARE was designed this way is:

- Manual evolutions simplify application logic by putting all complex evolutions & defaults in a conversion code instead of inside your core applciation logic
- Manual evolution forces you to handle edge cases of migrations & breaking changes at the cost of more verbose migration code
- Stop making big breaking v1 to v2 changes — instead, make much smaller schema changes with more flexibility
- Schema evolution frequently requires more than just renaming properties (like Protobuf, Flatbuffers, Cap'n'proto) — more complicated reshaping & fetching data from remote sources is commonly needed

## Implementations

- [TypeScript](./typescript/)
    - [Example Code](./typescript/examples/basic/src/migrator.ts)
- [Rust](./rust/)
    - [Example Code](./rust/examples/basic/src/lib.rs)

([Full list of BARE implementations](https://baremessages.org/))

_Adding an implementation takes less than an hour — it's really that simple._

## Current Users

- [Rivet Engine](https://github.com/rivet-dev/engine)
    - [Data at rest](https://github.com/rivet-dev/engine/tree/bbdf1c1c49e307ba252186aa4d75a9452d74fca7/sdks/schemas/data)
    - Internal network protocols ([tunnel](https://github.com/rivet-dev/engine/tree/bbdf1c1c49e307ba252186aa4d75a9452d74fca7/sdks/schemas/epoxy-protocol), [Epoxy](https://github.com/rivet-dev/engine/tree/bbdf1c1c49e307ba252186aa4d75a9452d74fca7/sdks/schemas/epoxy-protocol), [UPS](https://github.com/rivet-dev/engine/tree/bbdf1c1c49e307ba252186aa4d75a9452d74fca7/sdks/schemas/ups-protocol))
    - Public network protocol ([runner](https://github.com/rivet-dev/engine/tree/bbdf1c1c49e307ba252186aa4d75a9452d74fca7/sdks/schemas/runner-protocol))
- [RivetKit](https://github.com/rivet-dev/rivetkit)
    - [Client protocol](https://github.com/rivet-dev/rivetkit/tree/b81d9536ba7ccad4449639dd83a770eb7c353617/packages/rivetkit/schemas/client-protocol)
    - [Persisted state](https://github.com/rivet-dev/rivetkit/tree/b81d9536ba7ccad4449639dd83a770eb7c353617/packages/rivetkit/schemas/actor-persist)
    - [File system driver](https://github.com/rivet-dev/rivetkit/tree/b81d9536ba7ccad4449639dd83a770eb7c353617/packages/rivetkit/schemas/file-system-driver)

## Comparison with Other Formats

[Read more](./docs/COMPARISON.md)

## FAQ

### Why is copying the entire schema for every version better than using decorators for gradual migrations?

- Decorators are limited and become very complicated over time
- It's unclear at what version of the protocol a decorator takes effect — explicit versions help clarify this
- Generated SDKs become more and more bloated with every change
- You need a validation build step for your validators
- Manual migrations provide more flexibility for complex transformations

### Why not include RPC?

RPC interfaces are trivial to implement yourself. Libraries that provide RPC interfaces tend to add extra bloat and cognitive load through things like abstracting transports, compatibility with the language's async runtime, and complex codegen to implement handlers.

Usually, you just want a `ToServer` and `ToClient` union that looks like this: 
- [ToClient example](https://github.com/rivet-dev/rivetkit/blob/b81d9536ba7ccad4449639dd83a770eb7c353617/packages/rivetkit/schemas/client-protocol/v1.bare#L34)
- [ToServer example](https://github.com/rivet-dev/rivetkit/blob/b81d9536ba7ccad4449639dd83a770eb7c353617/packages/rivetkit/schemas/client-protocol/v1.bare#L56)

### Isn't copying the schema going to result in a lot of duplicate code?

Yes, but after enough pain and suffering from running production APIs, this is what you will end up doing manually anyway, but in a much more painful way. Having schema versions also makes it much easier to reason about how clients are connecting to your system and the state of an application. Incremental migrations don't let you consider other properties or structures. This approach also lets you reshape your structures more effectively.

### Don't migration steps get repetitive?

Most of the time, structures will match exactly, and most languages can provide a 1:1 migration. The most complicated migration steps will be for deeply nested structures that changed, but even that is relatively straightforward.

### What are the downsides?

- More verbose migration code — but this is usually because VBARE forces you to handle all edge cases
- The older the version, the more migration steps need that need to run to bring it to the latest version — though migration steps are usually negligible in cost
- Migration steps are not portable across languages, but only the server needs the migration steps, so this is usually only implemented once

## License

MIT

