import { describe, it, expect } from "vitest";
import * as V1 from "../dist/v1";
import * as V2 from "../dist/v2";
import * as V3 from "../dist/v3";
import { migrateToLatest, migrateV1ToV2App, migrateV2ToV3App } from "./migrator";

describe("basic migrator", () => {
  it("migrates v1 -> v2", () => {
    const v1: V1.App = {
      todos: [
        { id: 1, title: "task a", done: false },
        { id: 2, title: "task b", done: true },
      ],
    };
    const v2 = migrateV1ToV2App(v1);
    expect(v2.todos.size).toBe(2);
    const t1 = v2.todos.get(1n as V2.TodoId)!;
    const t2 = v2.todos.get(2n as V2.TodoId)!;
    expect(t1.title).toBe("task a");
    expect(t1.status).toBe(V2.TodoStatus.Open);
    expect(t1.createdAt).toBe(0n);
    expect(t1.tags).toEqual([]);
    expect(t2.status).toBe(V2.TodoStatus.Done);
    expect(v2.settings.size).toBe(0);
  });

  it("migrates v2 -> v3 (tags to map, defaults set)", () => {
    const todos = new Map<V2.TodoId, V2.Todo>();
    todos.set(10n as V2.TodoId, {
      id: 10n as V2.TodoId,
      title: "with tags",
      status: V2.TodoStatus.InProgress,
      createdAt: 123n as V2.u64,
      tags: ["red", "blue"],
    });
    const v2: V2.App = { todos, settings: new Map() };
    const v3 = migrateV2ToV3App(v2);
    expect(v3.todos.size).toBe(1);
    const t = v3.todos.get(10n as unknown as V3.TodoId)!;
    expect(t.detail.title).toBe("with tags");
    expect(t.detail.tags.size).toBe(2);
    expect(t.priority).toBe(V3.Priority.Medium);
    expect(t.assignee.kind).toBe(V3.AssigneeKind.None);
    expect(v3.config.theme).toBe(V3.Theme.System);
    expect(v3.boards.size).toBe(0);
  });

  it("migrates v1 -> v3 via migrateToLatest", () => {
    const v1: V1.App = {
      todos: [
        { id: 7, title: "hello", done: false },
      ],
    };
    const v3 = migrateToLatest(v1, 1);
    expect(v3.todos.size).toBe(1);
    const only = [...v3.todos.values()][0];
    expect(only.status).toBe(V3.TodoStatus.Open);
    expect(only.detail.title).toBe("hello");
  });
});

