use std::collections::HashMap;

use basic::{schemas, AppVersioned};
use vbare::OwnedVersionedData;

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

    let bytes = serde_bare::to_vec(&app_v1).unwrap();
    let migrated = AppVersioned::deserialize(&bytes, 1).unwrap();

    assert_eq!(migrated.todos.len(), 2);
    let done = migrated.todos.get(&2).unwrap();
    assert!(matches!(done.status, schemas::v3::TodoStatus::Done));
}

#[test]
fn migrates_v2_to_v3_with_tags() {
    let mut todos: HashMap<schemas::v2::TodoId, schemas::v2::Todo> = HashMap::new();
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
        settings: HashMap::new(),
    };
    let bytes = serde_bare::to_vec(&app_v2).unwrap();
    let migrated = AppVersioned::deserialize(&bytes, 2).unwrap();

    let t = migrated.todos.get(&5).unwrap();
    assert_eq!(t.detail.title, "with-tags");
    assert_eq!(t.detail.tags.len(), 2);
    assert_eq!(t.created_at, 42);
}

#[test]
fn serializes_v3_to_v1() {
    // Build a minimal v3::App with one DONE todo
    let mut todos: HashMap<schemas::v3::TodoId, schemas::v3::Todo> = HashMap::new();
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
                tags: HashMap::new(),
            },
            history: Vec::new(),
        },
    );

    let app_v3 = schemas::v3::App {
        todos,
        config: schemas::v3::AppConfig {
            theme: schemas::v3::Theme::System,
            features: HashMap::new(),
        },
        boards: HashMap::new(),
    };

    // Serialize to version 1 using the migrator's serialize path
    let bytes = AppVersioned::V3(app_v3).serialize(1).unwrap();

    // Decode as v1::App and assert down-conversion
    let app_v1: schemas::v1::App = serde_bare::from_slice(&bytes).unwrap();
    assert_eq!(app_v1.todos.len(), 1);
    let t = &app_v1.todos[0];
    assert_eq!(t.id, 7u32);
    assert_eq!(t.title, "hello");
    assert!(t.done);
}
