// Cross-checks conformance vectors against bare-ts.
//
// Usage: node check.mjs <schema-dir> <corpus.json>...
//
// <schema-dir> holds the bare-ts schema copies written by the conformance build script, as
// <set>/<module>.bare. Each corpus file is {"vectors": [...]}, where a vector is
//
//   {"schema": "<set>/<module>", "type": "<Type>", "value": <json>, "hex": "<bytes>"}
//   {"schema": "<set>/<module>", "type": "<Type>", "hex": "<bytes>", "invalid": "<reason>"}
//
// Every valid vector must encode from its value to exactly its bytes and decode from its bytes
// to exactly its value. Every invalid vector must be rejected.
//
// JSON value mapping, converted to and from bare-ts values by walking the bare-ts AST:
//
// - u64, i64, uint and int: decimal strings (bare-ts uses bigint); smaller integers: numbers;
// - f32 and f64: lowercase hex bit patterns of 8 or 16 digits;
// - data and data[N]: lowercase hex (bare-ts uses ArrayBuffer);
// - lists of fixed-width numbers: arrays (bare-ts uses typed arrays);
// - enums: the declared numeric value (bare-ts uses the PascalCase member name);
// - unions: {"tag": n, "value": v} (bare-ts uses {tag, val}, with the type name as the tag for
//   named members);
// - optionals and void: null;
// - maps: [key, value] pairs in encoding order (bare-ts uses Map, which keeps insertion order);
// - structs: objects keyed by schema field names, camelCased as in the schema copy.

import * as bare from "@bare-ts/lib";
import { Config, configure, parse, transform } from "@bare-ts/tools";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { isDeepStrictEqual } from "node:util";

const here = dirname(fileURLToPath(import.meta.url));
const [schemaDir, ...corpusFiles] = process.argv.slice(2);
if (schemaDir == null || corpusFiles.length === 0) {
    console.error("usage: node check.mjs <schema-dir> <corpus.json>...");
    process.exit(2);
}

// The same conversion as typescript/vbare-compiler and the build script.
function camelCase(name) {
    return name.replace(/_([a-zA-Z0-9])/g, (_, c) => c.toUpperCase());
}

// bare-ts's own CONSTANT_CASE to PascalCase conversion for enum member values.
function pascalCase(name) {
    const normalized = name
        .replace(/\B[A-Z][a-z\d]/g, (m) => `_${m}`)
        .replace(/[-_ ]+/g, "_");
    const camel = normalized.toLowerCase().replace(/_./g, (m) => m[1].toUpperCase());
    return camel.replace(/^./, (c) => c.toUpperCase());
}

const schemas = new Map();
let generatedCount = 0;

async function loadSchema(name) {
    let schema = schemas.get(name);
    if (schema != null) {
        return schema;
    }
    const source = readFileSync(join(schemaDir, `${name}.bare`), "utf8");
    const config = Config({ generator: "js" });
    const ast = configure(parse(source, config), config);
    // Generated modules import @bare-ts/lib, so they must live beside node_modules.
    const outDir = join(here, ".generated");
    mkdirSync(outDir, { recursive: true });
    const file = join(outDir, `${process.pid}-${generatedCount++}.mjs`);
    writeFileSync(file, transform(source, { generator: "js" }));
    const module = await import(pathToFileURL(file).href);
    const defs = new Map(ast.defs.map((def) => [def.alias, def.type]));
    schema = { module, defs };
    schemas.set(name, schema);
    return schema;
}

function resolve(defs, type) {
    while (type.tag === "alias") {
        type = defs.get(type.data);
    }
    return type;
}

function hexToBytes(hex) {
    const out = new Uint8Array(hex.length / 2);
    for (let i = 0; i < out.length; i++) {
        out[i] = Number.parseInt(hex.slice(2 * i, 2 * i + 2), 16);
    }
    return out;
}

function bytesToHex(bytes) {
    return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

function f32FromBits(hex) {
    const view = new DataView(new ArrayBuffer(4));
    view.setUint32(0, Number.parseInt(hex, 16));
    return view.getFloat32(0);
}

function f32Bits(value) {
    const view = new DataView(new ArrayBuffer(4));
    view.setFloat32(0, value);
    return view.getUint32(0).toString(16).padStart(8, "0");
}

function f64FromBits(hex) {
    const view = new DataView(new ArrayBuffer(8));
    view.setBigUint64(0, BigInt(`0x${hex}`));
    return view.getFloat64(0);
}

function f64Bits(value) {
    const view = new DataView(new ArrayBuffer(8));
    view.setFloat64(0, value);
    return view.getBigUint64(0).toString(16).padStart(16, "0");
}

const BIG_TAGS = new Set(["u64", "i64", "uint", "int"]);
const TYPED_ARRAYS = {
    f32: Float32Array,
    f64: Float64Array,
    i8: Int8Array,
    i16: Int16Array,
    i32: Int32Array,
    i64: BigInt64Array,
    u8: Uint8Array,
    u16: Uint16Array,
    u32: Uint32Array,
    u64: BigUint64Array,
};

function fromJson(defs, type, json) {
    if (type.tag === "alias") {
        return fromJson(defs, defs.get(type.data), json);
    }
    switch (type.tag) {
        case "bool":
        case "str":
            return json;
        case "f32":
            return f32FromBits(json);
        case "f64":
            return f64FromBits(json);
        case "data":
            return hexToBytes(json).buffer;
        case "void":
            return null;
        case "optional":
            return json === null ? null : fromJson(defs, type.types[0], json);
        case "list": {
            const element = type.types[0];
            if (type.extra?.typedArray) {
                const array = new TYPED_ARRAYS[element.tag](json.length);
                const view = new DataView(array.buffer);
                json.forEach((item, i) => {
                    // Set float elements by bit pattern so NaN payloads survive.
                    if (element.tag === "f32") {
                        view.setUint32(4 * i, Number.parseInt(item, 16), true);
                    } else if (element.tag === "f64") {
                        view.setBigUint64(8 * i, BigInt(`0x${item}`), true);
                    } else {
                        array[i] = fromJson(defs, element, item);
                    }
                });
                return array;
            }
            return json.map((item) => fromJson(defs, element, item));
        }
        case "map":
            return new Map(
                json.map(([key, value]) => [
                    fromJson(defs, type.types[0], key),
                    fromJson(defs, type.types[1], value),
                ]),
            );
        case "enum": {
            const member = type.data.find((m) => m.val === json);
            if (member == null) {
                throw new Error(`no enum member with value ${json}`);
            }
            return pascalCase(member.name);
        }
        case "union": {
            const index = type.data.findIndex((m) => m.val === json.tag);
            if (index < 0) {
                throw new Error(`no union member with tag ${json.tag}`);
            }
            const member = type.types[index];
            const tag = member.tag === "alias" ? member.data : json.tag;
            return { tag, val: fromJson(defs, member, json.value) };
        }
        case "struct": {
            const out = {};
            type.data.forEach((field, i) => {
                out[field.name] = fromJson(defs, type.types[i], json[field.name]);
            });
            return out;
        }
        default:
            if (BIG_TAGS.has(type.tag)) {
                return BigInt(json);
            }
            return json;
    }
}

function toJson(defs, type, value) {
    if (type.tag === "alias") {
        return toJson(defs, defs.get(type.data), value);
    }
    switch (type.tag) {
        case "bool":
        case "str":
            return value;
        case "f32":
            return f32Bits(value);
        case "f64":
            return f64Bits(value);
        case "data":
            return bytesToHex(new Uint8Array(value));
        case "void":
            return null;
        case "optional":
            return value === null ? null : toJson(defs, type.types[0], value);
        case "list": {
            const element = type.types[0];
            if (type.extra?.typedArray && (element.tag === "f32" || element.tag === "f64")) {
                const view = new DataView(value.buffer, value.byteOffset, value.byteLength);
                return Array.from(value, (_, i) =>
                    element.tag === "f32"
                        ? view.getUint32(4 * i, true).toString(16).padStart(8, "0")
                        : view.getBigUint64(8 * i, true).toString(16).padStart(16, "0"),
                );
            }
            return Array.from(value, (item) => toJson(defs, element, item));
        }
        case "map":
            return Array.from(value, ([key, item]) => [
                toJson(defs, type.types[0], key),
                toJson(defs, type.types[1], item),
            ]);
        case "enum": {
            const member = type.data.find((m) => pascalCase(m.name) === value);
            if (member == null) {
                throw new Error(`no enum member named ${value}`);
            }
            return member.val;
        }
        case "union": {
            const index = type.types.findIndex((member, i) =>
                member.tag === "alias" ? member.data === value.tag : type.data[i].val === value.tag,
            );
            if (index < 0) {
                throw new Error(`no union member for tag ${value.tag}`);
            }
            return { tag: type.data[index].val, value: toJson(defs, type.types[index], value.val) };
        }
        case "struct": {
            const out = {};
            type.data.forEach((field, i) => {
                out[field.name] = toJson(defs, type.types[i], value[field.name]);
            });
            return out;
        }
        default:
            if (BIG_TAGS.has(type.tag)) {
                return value.toString();
            }
            return value;
    }
}

/// Renames struct fields in a corpus value to their camelCase schema-copy names.
function camelFields(defs, type, json) {
    type = resolve(defs, type);
    if (json === null) {
        return json;
    }
    switch (type.tag) {
        case "optional":
            return camelFields(defs, type.types[0], json);
        case "list":
            return json.map((item) => camelFields(defs, type.types[0], item));
        case "map":
            return json.map(([key, value]) => [key, camelFields(defs, type.types[1], value)]);
        case "union": {
            const index = type.data.findIndex((m) => m.val === json.tag);
            return index < 0
                ? json
                : { tag: json.tag, value: camelFields(defs, type.types[index], json.value) };
        }
        case "struct": {
            const out = {};
            const byCamel = new Map(Object.keys(json).map((key) => [camelCase(key), key]));
            type.data.forEach((field, i) => {
                out[field.name] = camelFields(defs, type.types[i], json[byCamel.get(field.name)]);
            });
            return out;
        }
        default:
            return json;
    }
}

function newCursor(bytes) {
    return new bare.ByteCursor(bytes, bare.Config({}));
}

let checked = 0;
const failures = [];
for (const corpusFile of corpusFiles) {
    const corpus = JSON.parse(readFileSync(corpusFile, "utf8"));
    for (const vector of corpus.vectors) {
        const label = `${vector.schema}::${vector.type} ${vector.hex}`;
        try {
            const { module, defs } = await loadSchema(vector.schema);
            const type = { tag: "alias", data: vector.type };
            const read = module[`read${vector.type}`];
            const write = module[`write${vector.type}`];
            if (read == null || write == null) {
                throw new Error(`bare-ts exports no reader or writer for ${vector.type}`);
            }
            const bytes = hexToBytes(vector.hex);
            if (vector.invalid != null) {
                let accepted = false;
                try {
                    const bc = newCursor(bytes);
                    read(bc);
                    accepted = bc.offset === bytes.length;
                } catch {
                    accepted = false;
                }
                if (accepted) {
                    failures.push(`${label}: accepted invalid input (${vector.invalid})`);
                }
                checked++;
                continue;
            }
            const expected = camelFields(defs, type, vector.value);

            const bc = new bare.ByteCursor(new Uint8Array(64), bare.Config({}));
            write(bc, fromJson(defs, type, expected));
            const encoded = bytesToHex(new Uint8Array(bc.view.buffer, bc.view.byteOffset, bc.offset));
            if (encoded !== vector.hex) {
                failures.push(`${label}: bare-ts encoded ${encoded}`);
            }

            const reader = newCursor(bytes);
            const decoded = toJson(defs, type, read(reader));
            if (reader.offset !== bytes.length) {
                failures.push(`${label}: bare-ts left ${bytes.length - reader.offset} bytes`);
            } else if (!isDeepStrictEqual(decoded, expected)) {
                failures.push(
                    `${label}: bare-ts decoded ${JSON.stringify(decoded)}, expected ${JSON.stringify(expected)}`,
                );
            }
            checked++;
        } catch (error) {
            failures.push(`${label}: ${error.stack ?? error}`);
        }
    }
}

console.log(`bare-ts checked ${checked} vectors, ${failures.length} failures`);
if (failures.length > 0) {
    console.error(failures.slice(0, 50).join("\n"));
    process.exit(1);
}
