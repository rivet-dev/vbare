use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use vbare::OwnedVersionedData;

// Bring generated schemas into this crate
pub mod schemas {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/combined_imports.rs"));
}

// A simple versioned wrapper for App across v1, v2, v3.
#[derive(Clone)]
pub enum AppVersioned {
    V1(schemas::v1::App),
    V2(schemas::v2::App),
    V3(schemas::v3::App),
}

impl vbare::OwnedVersionedData for AppVersioned {
    type Latest = schemas::v3::App;

    fn latest(latest: Self::Latest) -> Self {
        AppVersioned::V3(latest)
    }

    fn into_latest(self) -> Result<Self::Latest> {
        match self {
            AppVersioned::V3(app) => Ok(app),
            _ => bail!("version not latest"),
        }
    }

    fn deserialize_version(payload: &[u8], version: u16) -> Result<Self> {
        match version {
            1 => Ok(AppVersioned::V1(serde_bare::from_slice(payload)?)),
            2 => Ok(AppVersioned::V2(serde_bare::from_slice(payload)?)),
            3 => Ok(AppVersioned::V3(serde_bare::from_slice(payload)?)),
            _ => bail!("invalid version: {version}"),
        }
    }

    fn serialize_version(self, _version: u16) -> Result<Vec<u8>> {
        match self {
            AppVersioned::V1(data) => Ok(serde_bare::to_vec(&data)?),
            AppVersioned::V2(data) => Ok(serde_bare::to_vec(&data)?),
            AppVersioned::V3(data) => Ok(serde_bare::to_vec(&data)?),
        }
    }

    fn deserialize_converters() -> Vec<impl Fn(Self) -> Result<Self>> {
        vec![Self::v1_to_v2, Self::v2_to_v3]
    }

    fn serialize_converters() -> Vec<impl Fn(Self) -> Result<Self>> {
        // Not used by this example, but keep symmetric
        vec![Self::v3_to_v2, Self::v2_to_v1]
    }
}

impl AppVersioned {
    fn v1_to_v2(self) -> Result<Self> {
        use schemas::{v1, v2};
        match self {
            AppVersioned::V1(app) => {
                let mut todos: std::collections::HashMap<v2::TodoId, v2::Todo> = Default::default();
                for t in app.todos.into_iter() {
                    let id: v2::TodoId = t.id as u64;
                    let status = if t.done {
                        v2::TodoStatus::Done
                    } else {
                        v2::TodoStatus::Open
                    };
                    let todo = v2::Todo {
                        id,
                        title: t.title,
                        status,
                        created_at: 0,
                        tags: Vec::new(),
                    };
                    todos.insert(id, todo);
                }
                let app_v2 = v2::App {
                    todos,
                    settings: Default::default(),
                };
                Ok(AppVersioned::V2(app_v2))
            }
            other => Ok(other),
        }
    }

    fn v2_to_v3(self) -> Result<Self> {
        use schemas::{v2, v3};
        match self {
            AppVersioned::V2(app) => {
                // Convert tags: Vec<String> -> HashMap<TagId, Tag>
                fn convert_tags(
                    tags: Vec<String>,
                ) -> std::collections::HashMap<v3::TagId, v3::Tag> {
                    let mut map = std::collections::HashMap::new();
                    let mut next_id: v3::TagId = 1; // simple incremental
                    for name in tags.into_iter() {
                        let tag = v3::Tag {
                            id: next_id,
                            name,
                            color: None,
                        };
                        map.insert(next_id, tag);
                        next_id += 1;
                    }
                    map
                }

                let mut todos: std::collections::HashMap<v3::TodoId, v3::Todo> = Default::default();
                for (id, t) in app.todos.into_iter() {
                    let detail = v3::TodoDetail {
                        title: t.title,
                        tags: convert_tags(t.tags),
                    };
                    let status = match t.status {
                        v2::TodoStatus::Open => v3::TodoStatus::Open,
                        v2::TodoStatus::InProgress => v3::TodoStatus::InProgress,
                        v2::TodoStatus::Done => v3::TodoStatus::Done,
                    };
                    let todo = v3::Todo {
                        id,
                        status,
                        created_at: t.created_at,
                        priority: v3::Priority::Low,
                        assignee: v3::Assignee {
                            kind: v3::AssigneeKind::None,
                            user_id: None,
                            team_id: None,
                        },
                        detail,
                        history: Vec::new(),
                    };
                    todos.insert(id, todo);
                }

                let app_v3 = v3::App {
                    todos,
                    config: v3::AppConfig {
                        theme: v3::Theme::System,
                        features: Default::default(),
                    },
                    boards: Default::default(),
                };
                Ok(AppVersioned::V3(app_v3))
            }
            other => Ok(other),
        }
    }

    fn v3_to_v2(self) -> Result<Self> {
        use schemas::{v2, v3};
        match self {
            AppVersioned::V3(app) => {
                fn revert_tags(tags: std::collections::HashMap<v3::TagId, v3::Tag>) -> Vec<String> {
                    let mut v = Vec::with_capacity(tags.len());
                    for (_, tag) in tags.into_iter() {
                        v.push(tag.name);
                    }
                    v
                }
                let mut todos: std::collections::HashMap<v2::TodoId, v2::Todo> = Default::default();
                for (id, t) in app.todos.into_iter() {
                    let title = t.detail.title;
                    let tags = revert_tags(t.detail.tags);
                    let status = match t.status {
                        v3::TodoStatus::Open => v2::TodoStatus::Open,
                        v3::TodoStatus::InProgress => v2::TodoStatus::InProgress,
                        v3::TodoStatus::Done => v2::TodoStatus::Done,
                    };
                    todos.insert(
                        id,
                        v2::Todo {
                            id,
                            title,
                            status,
                            created_at: t.created_at,
                            tags,
                        },
                    );
                }
                let app_v2 = v2::App {
                    todos,
                    settings: Default::default(),
                };
                Ok(AppVersioned::V2(app_v2))
            }
            other => Ok(other),
        }
    }

    fn v2_to_v1(self) -> Result<Self> {
        use schemas::{v1, v2};
        match self {
            AppVersioned::V2(app) => {
                let mut todos: Vec<v1::Todo> = Vec::new();
                for (_id, t) in app.todos.into_iter() {
                    let done = matches!(t.status, v2::TodoStatus::Done);
                    todos.push(v1::Todo {
                        id: t.id as u32,
                        title: t.title,
                        done,
                    });
                }
                Ok(AppVersioned::V1(v1::App { todos }))
            }
            other => Ok(other),
        }
    }
}

// Convenience function for consumers/tests
pub fn migrate_to_latest(payload: &[u8], version: u16) -> Result<schemas::v3::App> {
    AppVersioned::deserialize(payload, version)
}
