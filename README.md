# Versioned Binary Application Record Encoding (VBARE)

Fearless schema migrations at theoretical maximum performance

VBARE is a tiny extension to BARE

## Preface: What is BARE?

- https://baremessages.org/
- https://www.ietf.org/archive/id/draft-devault-bare-11.html

## Project goals

- fast -- self-contained binary encoding, akin to a tuple -> 
- simple -- can rewrite in under an hour
- portable -- cross-language & well standardized

non-goals:

- data compactness -> that's what gzip is for

## Use cases

- Defining network protocols
- Storing data at rest that needs to be able to be upgraded
    - Binary data in the database
    - File formats

## At a glance

- Every message has a version associated with it
    - either pre-negotiated (via something like an http request query parameter/handshake) or embedded int he message itself
- Applications provide functions to upgrade between protocol versions
- There is no migration in the schema itself, just copy and paste the schema to write the new one

## Migration philosophy

- declare discrete versions with predefined version indexes
- manual migrations simplify the application logic by putting complex defaults in your app code
- stop making big breaking v1 -> v2 changes, make much smaller changes with more flexibility
- reshaping structures is important -- not just changing types and names

## Code examples

## Current users

- Rivet
    - Network protocol
    - All internal communication
    - All data stored at rest
- RivetKit
    - Protocol for communicating with clients

## FAQ

### Why not include RPC?

- why the fuck does your protocol need to define an rpc schema
- keep it simple, use a union

### Why is copying the entire schema for every version better than using decorators for gradual migrations

### Isn't copying the schema going to result in a lot of duplicate code?

- yes. after enough pain and suffering of running production APIS, this is what you will end up doing manually, but in a much more painful way.
- having schema versions also makes it much easier to reason about how clients are connecting to your system/the state of an application. incremental migrations dno't let you consider other properties/structures.
- this also lets you reshape your structures.

### Why copying instead of decorators for migrations?

- decorators are limited and get very complicated
- it's unclear what version of the protocol a decorator takes effect -- this is helpful
- generated sdks become more and more bloated with every change
- you need a validation build step for your validators
- things you can do with manual migrations

### Don't migration steps get repetitive?

- most of the time, structures will match exactly. most languages can provide a 1:1 migration.
- the most eggrarious offendors will be deeply nested structures, but even that isn't terrible

## Comparison

- Protobuf (versioned: yes)
    - unbelievably poorly designed protocol
    - makes it your problem by making everything optional
    - even worse, makes properties have a default value (ie integers) which leads to subtle bugs with serious concequenses
    - tracking indexes in the file is ass
- Cap'n'proto (versioned: yes)
    - simplicity
    - quality of output languages
- cbor/messagepack/that mongodb one (versioned: self-describing)
    - requires encoding the entire key
- Flatbuffers (versioned: yes)
    - still uses indexes like protobuf, unless you use structs
    - structs are ass
    - cdoegen is ass
- https://crates.io/crates/bebop & https://crates.io/crates/borsh (versioned: TODO)
    - provides cross platform
    - TODO: more complicated than i'd like
    - bebop includes an extra ??? step
- rust options like postcard/etc (versioned: no)
    - not cross platform 

## Implementations

| Language | BARE | VBARE |
| --- | --- | --- |
| TypeScript | X | X |
| Rust | X | X |

[Full list of BARE implementations](https://baremessages.org/)

