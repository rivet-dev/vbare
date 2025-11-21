# TypeScript

## Packages

- `vbare`: Runtime helpers exposed as the `vbare` package. The primary entry point is `createVersionedDataHandler`, which mirrors the Rust `OwnedVersionedData` helpers.
    - [npm package: `vbare`](https://www.npmjs.com/package/vbare)
- `@vbare/compiler`: CLI + library published as `@vbare/compiler`. It wraps `@bare-ts/tools` and adds light preprocessing so fixtures compile without extra flags.
- `@vbare/example-basic`: End-to-end sample that compiles schemas and wires migrations together. Generated sources live in `examples/basic/dist/` and hand-written logic in `examples/basic/src/`.

## Quick Start

**Step 1: Add dependencies to your workspace:**

```json
{
  "dependencies": {
    "vbare": "0.0.4"
  },
  "devDependencies": {
    "@vbare/compiler": "0.0.4"
  }
}
```

**Step 2: Add a build script to your `package.json`:**

```json
{
  "scripts": {
    "bare:build": "vbare-compiler schemas --out-dir src/dist"
  }
}
```

**Step 3: Compile your `.bare` schema files into TypeScript modules:**

```bash
pnpm bare:build
```

**Step 4: Import the generated modules in your project:**

```ts
import * as V1 from "./dist/v1";
import * as V2 from "./dist/v2";
import * as V3 from "./dist/v3";
```

**Step 5: Create a handler that understands every version:**

```ts
import { createVersionedDataHandler } from "vbare";

export const AppHandler = createVersionedDataHandler<V3.App>({
  deserializeVersion: (bytes, version) => {
    switch (version) {
      case 1: return V1.decodeApp(bytes);
      case 2: return V2.decodeApp(bytes);
      case 3: return V3.decodeApp(bytes);
      default: throw new Error(`invalid version: ${version}`);
    }
  },
  serializeVersion: (data, version) => {
    switch (version) {
      case 1: return V1.encodeApp(data as V1.App);
      case 2: return V2.encodeApp(data as V2.App);
      case 3: return V3.encodeApp(data as V3.App);
      default: throw new Error(`invalid version: ${version}`);
    }
  },
  deserializeConverters: () => [migrateV1ToV2App, migrateV2ToV3App],
  serializeConverters: () => [migrateV3ToV2App, migrateV2ToV1App],
});
```

Then use `deserialize`/`serialize` or their `*WithEmbeddedVersion` variants:

```ts
// Decode bytes encoded as version 1 into latest
const latest = AppHandler.deserialize(bytes, 1);

// Encode latest as version 1
const v1Bytes = AppHandler.serialize(latest, 1);

// Or embed version in the payload header (little-endian u16 prefix)
const payload = AppHandler.serializeWithEmbeddedVersion(latest, 3);
const latest2 = AppHandler.deserializeWithEmbeddedVersion(payload);
```

## License

MIT
