import * as V1 from "../dist/v1";
import * as V2 from "../dist/v2";
import * as V3 from "../dist/v3";
import { createVersionedDataHandler, type MigrationFn } from "vbare";

export function migrateV1TodoToV2(todo: V1.Todo): V2.Todo {
  return {
    id: BigInt(todo.id) as V2.TodoId,
    title: todo.title,
    status: todo.done ? V2.TodoStatus.Done : V2.TodoStatus.Open,
    createdAt: 0n as V2.u64,
    tags: [],
  } as V2.Todo;
}

export function migrateV1ToV2App(app: V1.App): V2.App {
  const todos = new Map<V2.TodoId, V2.Todo>();
  for (const t of app.todos) {
    const migrated = migrateV1TodoToV2(t);
    todos.set(migrated.id, migrated);
  }
  return {
    todos,
    settings: new Map<string, string>(),
  } as V2.App;
}

export function migrateV2TodoToV3(todo: V2.Todo): V3.Todo {
  // Convert list<string> tags to map<TagId, Tag>
  const tags = new Map<V3.TagId, V3.Tag>();
  let nextTagId = 1 as V3.TagId; // simple incremental ids
  for (const name of todo.tags) {
    const id = nextTagId as V3.TagId;
    tags.set(id, { id, name, color: null });
    nextTagId = ((nextTagId as unknown as number) + 1) as V3.TagId;
  }

  return {
    id: todo.id as unknown as V3.TodoId,
    status: (todo.status as unknown) as V3.TodoStatus,
    createdAt: todo.createdAt as unknown as V3.u64,
    priority: V3.Priority.Medium,
    assignee: { kind: V3.AssigneeKind.None, userId: null, teamId: null },
    detail: { title: todo.title, tags },
    history: [],
  } as V3.Todo;
}

export function migrateV2ToV3App(app: V2.App): V3.App {
  const todos = new Map<V3.TodoId, V3.Todo>();
  for (const [id, t] of app.todos) {
    const migrated = migrateV2TodoToV3(t);
    todos.set(id as unknown as V3.TodoId, migrated);
  }

  return {
    todos,
    config: { theme: V3.Theme.System, features: new Map<string, boolean>() },
    boards: new Map<V3.BoardId, V3.Board>(),
  } as V3.App;
}

// Set up versioned migration handler using the vbare package.
export const CURRENT_VERSION = 3 as const;

// Map migrations as fromVersion -> (data) => nextVersionData
export const migrations = new Map<number, MigrationFn<any, any>>([
  [1, (data: V1.App) => migrateV1ToV2App(data)],
  [2, (data: V2.App) => migrateV2ToV3App(data)],
]);

// For this example we use JSON for (de)serialization to drive the migration flow.
// The focus is on demonstrating the vbare migration wiring, not binary I/O.
const jsonEncoder = new TextEncoder();
const jsonDecoder = new TextDecoder();

export const APP_VERSIONED = createVersionedDataHandler<V3.App>({
  currentVersion: CURRENT_VERSION,
  migrations,
  serializeVersion: (data: V3.App) => jsonEncoder.encode(JSON.stringify(data)),
  deserializeVersion: (bytes: Uint8Array) => JSON.parse(jsonDecoder.decode(bytes)),
});

export function migrateToLatest(
  app: V1.App | V2.App | V3.App,
  fromVersion: 1 | 2 | 3,
): V3.App {
  if (fromVersion === 3) return app as V3.App;
  // Use the versioned handler to apply migrations starting from fromVersion.
  const bytes = jsonEncoder.encode(JSON.stringify(app));
  return APP_VERSIONED.deserialize(bytes, fromVersion);
}
