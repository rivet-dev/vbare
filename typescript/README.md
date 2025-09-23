# TypeScript Packages

This workspace contains the TypeScript implementation of VBARE. It includes the runtime used to upgrade and downgrade BARE payloads, a tiny compiler that turns `.bare` schemas into TypeScript helpers, and an example that exercises the full migration flow.

## Workspace layout

- `vbare/` – runtime helpers exposed as the `vbare` package. The primary surface is `createVersionedDataHandler`, which mirrors the Rust `OwnedVersionedData` helpers.
- `vbare-compiler/` – CLI + library published as `@vbare/compiler`. It wraps `@bare-ts/tools` and adds light preprocessing so the fixtures in `fixtures/tests` compile without extra flags.
- `examples/basic/` – end-to-end sample that compiles schemas and wires migrations together. The generated sources live in `examples/basic/dist/` and the hand-written logic is under `examples/basic/src/`.

`pnpm-workspace.yaml` registers the three packages above so you can target them with `pnpm --filter`.

## Bootstrapping & common tasks

From `typescript/`:

```bash
pnpm install              # install workspace dependencies
pnpm build                # build compiler + runtime via turbo (tsup)
pnpm test                 # run vitest suites for all workspace packages
pnpm --filter vbare dev   # optional: rebuild runtime on changes
```

Each package also exposes the usual `build`, `dev`, `test`, and `check-types` scripts if you need to run them in isolation (e.g. `pnpm --filter @vbare/compiler test`).

## Using the runtime (`vbare`)

`VersionedDataHandler` encapsulates the logic for moving between schema versions. You provide raw serializers for every version plus conversion chains that step forward (deserialize) or backward (serialize). The helper can also embed / extract a 16-bit version prefix so payloads match the Rust helpers.

```ts
import { createVersionedDataHandler } from "vbare";
import * as V1 from "./dist/v1";
import * as V2 from "./dist/v2";
import * as V3 from "./dist/v3";

const AppHandler = createVersionedDataHandler<V3.App>({
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

const latest = AppHandler.deserializeWithEmbeddedVersion(bytes);
const downgraded = AppHandler.serialize(latest, 1);
```

The example above is lifted from `examples/basic/src/index.ts`. Notice how migrations are typed at the boundary and erased when they enter the handler, mirroring the Rust API.

## Compiling schemas (`@vbare/compiler`)

The compiler accepts either a single `.bare` file or a folder full of versioned schemas. Preprocessing normalises comment styles, `map<A, B>` syntax, and snake_case field names so they align with the stricter `@bare-ts/tools` parser.

Common workflows:

```bash
# Compile every schema in a directory into dist/*.ts
pnpm --filter @vbare/compiler exec -- vbare-compiler fixtures/tests/basic --out-dir examples/basic/dist

# Compile a single schema
pnpm --filter @vbare/compiler exec -- vbare-compiler fixtures/tests/basic/v3.bare --output tmp/v3.ts
```

You can also import `compileSchema` directly if you want to wire the transformation into a build script.

## Example project

`examples/basic` consumes the fixtures in `fixtures/tests/basic`, runs them through the compiler, and wires migrations together:

```bash
pnpm --filter @vbare/example-basic build  # regenerates dist/v*.ts
pnpm --filter @vbare/example-basic test   # runs vitest assertions against migrations
```

The tests (`examples/basic/index.test.ts`) cover both upgrade (v1/v2 → v3) and downgrade (v3 → v1) flows using the `APP_VERSIONED` handler defined in `examples/basic/src/index.ts`.

Running the example is the quickest way to see the runtime and compiler working together end-to-end.
