
## How we got here

- updating apis is incredibly error prone
- we would frequently find ourselves catching breaking changes by:
    - adding request properties that were not optional
    - adding enum variants in responses that clients were not adept to handle
- to solve this, we:
    - started frequently copying and pasting our entire api router
    - the latest version keeps the business logic
    - the previous version would include logic to mutate the schema to match the newest
        - this frequently involved making database queries to mutate data to match the new API
- additional pain points:
    - we kept using protobuf because it was the only option with descent code generation that would let us mutate the schema
    - but we knew it was terrible in how it handled version migration
    - kept introducing edge cases with default values
- how vbare fixes this:
    - builds infrastructure around the pattern that emerged: making micromigrations for many small changes
    - provides custom hooks to handle migrations between versions

