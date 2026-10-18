use std::collections::BTreeMap;

use basic::{migrate_to_latest, schemas, AppVersioned};
use vbare::{Encode, Message, Versioned};

#[test]
fn migrates_v1_to_v3() {
    let app_v1 = schemas::v1::App {
        todos: vec![
            schemas::v1::Todo {
                id: 1,
                title: "a".into(),
                done: false,
            },
            schemas::v1::Todo {
                id: 2,
                title: "b".into(),
                done: true,
            },
        ],
    };

    let migrated = migrate_to_latest(app_v1.encode_to_bytes(), 1).unwrap();

    assert_eq!(migrated.todos.len(), 2);
    let done = migrated.todos.get(&2).unwrap();
    assert!(matches!(done.status, schemas::v3::TodoStatus::Done));
}

#[test]
fn migrates_v2_to_v3_with_tags() {
    let mut todos = BTreeMap::new();
    todos.insert(
        5,
        schemas::v2::Todo {
            id: 5,
            title: "with-tags".into(),
            status: schemas::v2::TodoStatus::Open,
            created_at: 42,
            tags: vec!["red".into(), "blue".into()],
        },
    );
    let app_v2 = schemas::v2::App {
        todos,
        settings: BTreeMap::new(),
    };
    let migrated = migrate_to_latest(app_v2.encode_to_bytes(), 2).unwrap();

    let t = migrated.todos.get(&5).unwrap();
    assert_eq!(t.detail.title, "with-tags");
    assert_eq!(t.detail.tags.len(), 2);
    assert_eq!(t.created_at, 42);
}

#[test]
fn serializes_v3_to_v1() {
    let mut todos = BTreeMap::new();
    todos.insert(
        7,
        schemas::v3::Todo {
            id: 7,
            status: schemas::v3::TodoStatus::Done,
            created_at: 123,
            priority: schemas::v3::Priority::High,
            assignee: schemas::v3::Assignee {
                kind: schemas::v3::AssigneeKind::None,
                user_id: None,
                team_id: None,
            },
            detail: schemas::v3::TodoDetail {
                title: "hello".into(),
                tags: BTreeMap::new(),
            },
            history: Vec::new(),
        },
    );

    let app_v3 = schemas::v3::App {
        todos,
        config: schemas::v3::AppConfig {
            theme: schemas::v3::Theme::System,
            features: BTreeMap::new(),
        },
        boards: BTreeMap::new(),
    };

    // Seal at version 1, which runs the down-converters.
    let bytes = AppVersioned::seal_to_vec(&app_v3, 1).unwrap();
    assert_eq!(bytes[..2], [1, 0]);

    let app_v1 = schemas::v1::App::decode(bytes[2..].to_vec().into()).unwrap();
    assert_eq!(app_v1.todos.len(), 1);
    let t = &app_v1.todos[0];
    assert_eq!(t.id, 7u32);
    assert_eq!(t.title, "hello");
    assert!(t.done);
}

#[test]
fn opens_latest_payloads_in_place() {
    let app = schemas::v3::App {
        todos: BTreeMap::new(),
        config: schemas::v3::AppConfig {
            theme: schemas::v3::Theme::Dark,
            features: BTreeMap::from([("beta".into(), true)]),
        },
        boards: BTreeMap::new(),
    };
    let bytes = AppVersioned::seal_to_vec(&app, 3).unwrap();
    let view = AppVersioned::open(bytes.into()).unwrap();
    let config = view.get().config();
    assert_eq!(config.theme(), schemas::v3::Theme::Dark);
    assert_eq!(config.features().get("beta"), Some(true));
    assert_eq!(view.to_owned(), app);
}
