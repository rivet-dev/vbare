import * as V1 from "../dist/v1";
import * as V2 from "../dist/v2";
import * as V3 from "../dist/v3";
import { createVersionedDataHandler } from "vbare";

export const CURRENT_VERSION = 3 as const;

export const APP_VERSIONED = createVersionedDataHandler<V3.App>({
	deserializeVersion: (bytes: Uint8Array, version: number): any => {
		switch (version) {
			case 1:
				return V1.decodeApp(bytes);
			case 2:
				return V2.decodeApp(bytes);
			case 3:
				return V3.decodeApp(bytes);
			default:
				throw new Error(`invalid version: ${version}`);
		}
	},
	serializeVersion: (data: any, version: number): Uint8Array => {
		switch (version) {
			case 1:
				return V1.encodeApp(data as V1.App);
			case 2:
				return V2.encodeApp(data as V2.App);
			case 3:
				return V3.encodeApp(data as V3.App);
			default:
				throw new Error(`invalid version: ${version}`);
		}
	},
	// Use typed migrations and erase types at the array boundary
	deserializeConverters: () =>
		([migrateV1ToV2App, migrateV2ToV3App] as unknown as Array<(data: any) => any>),
	serializeConverters: () =>
		([migrateV3ToV2App, migrateV2ToV1App] as unknown as Array<(data: any) => any>),
});

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
		status: todo.status as unknown as V3.TodoStatus,
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


// Optional downgrades to mirror Rust API
export function migrateV3ToV2App(app: V3.App): V2.App {
	const todos = new Map<V2.TodoId, V2.Todo>();
	for (const [id, t] of app.todos) {
		const tags: string[] = [];
		for (const [, tag] of t.detail.tags) tags.push(tag.name);
		const status = t.status as unknown as V2.TodoStatus;
		todos.set(id as unknown as V2.TodoId, {
			id: id as unknown as V2.TodoId,
			title: t.detail.title,
			status,
			createdAt: t.createdAt as unknown as V2.u64,
			tags,
		});
	}
	return { todos, settings: new Map<string, string>() } as V2.App;
}

export function migrateV2ToV1App(app: V2.App): V1.App {
	const todos: V1.Todo[] = [];
	for (const [, t] of app.todos) {
		const done = t.status === V2.TodoStatus.Done;
		todos.push({ id: Number(t.id) as V1.TodoId, title: t.title, done });
	}
	return { todos } as V1.App;
}
