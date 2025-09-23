import { describe, it, expect } from "vitest";
import * as V1 from "./dist/v1";
import * as V2 from "./dist/v2";
import * as V3 from "./dist/v3";
import { APP_VERSIONED } from "./src/index.ts";

describe("example basic migrations", () => {
  it("migrates v1 -> v3", () => {
    const appV1: V1.App = {
      todos: [
        { id: 1 as V1.TodoId, title: "a", done: false },
        { id: 2 as V1.TodoId, title: "b", done: true },
      ],
    };

    const bytes = V1.encodeApp(appV1);
    const migrated = APP_VERSIONED.deserialize(bytes, 1);

    expect(migrated.todos.size).toBe(2);
    const done = migrated.todos.get(2n as V3.TodoId)!;
    expect(done.status).toBe(V3.TodoStatus.Done);
  });

  it("migrates v2 -> v3 with tags", () => {
    const todos = new Map<V2.TodoId, V2.Todo>();
    todos.set(5n as V2.TodoId, {
      id: 5n as V2.TodoId,
      title: "with-tags",
      status: V2.TodoStatus.Open,
      createdAt: 42n as V2.u64,
      tags: ["red", "blue"],
    });
    const appV2: V2.App = { todos, settings: new Map<string, string>() };

    const bytes = V2.encodeApp(appV2);
    const migrated = APP_VERSIONED.deserialize(bytes, 2);

    const t = migrated.todos.get(5n as V3.TodoId)!;
    expect(t.detail.title).toBe("with-tags");
    expect(t.detail.tags.size).toBe(2);
    expect(t.createdAt).toBe(42n);
  });

  it("serializes v3 -> v1 (downgrade)", () => {
    // Build a minimal v3::App with one DONE todo
    const todos = new Map<V3.TodoId, V3.Todo>();
    todos.set(7 as V3.TodoId, {
      id: 7 as V3.TodoId,
      status: V3.TodoStatus.Done,
      createdAt: 123n as V3.u64,
      priority: V3.Priority.High,
      assignee: { kind: V3.AssigneeKind.None, userId: null, teamId: null },
      detail: { title: "hello", tags: new Map<V3.TagId, V3.Tag>() },
      history: [],
    });
    const appV3: V3.App = {
      todos,
      config: { theme: V3.Theme.System, features: new Map<string, boolean>() },
      boards: new Map<V3.BoardId, V3.Board>(),
    };

    // Serialize to version 1 via handler and decode as V1
    const bytes = APP_VERSIONED.serialize(appV3, 1);
    const appV1 = V1.decodeApp(bytes);
    expect(appV1.todos.length).toBe(1);
    const t = appV1.todos[0];
    expect(t.id).toBe(7 as unknown as number);
    expect(t.title).toBe("hello");
    expect(t.done).toBe(true);
  });
});
