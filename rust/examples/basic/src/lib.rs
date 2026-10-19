use anyhow::{bail, Result};
use std::collections::BTreeMap;
use vbare::{Bytes, DecodeError, DecodeErrorKind, Encode, EncodeError, Message, Sink, Versioned};

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

impl Versioned for AppVersioned {
    type Latest = schemas::v3::App;

    const LATEST: u16 = 3;
    const UPGRADES: &'static [vbare::Converter<Self>] = &[Self::v1_to_v2, Self::v2_to_v3];
    const DOWNGRADES: &'static [vbare::Converter<Self>] = &[Self::v3_to_v2, Self::v2_to_v1];

    fn wrap_latest(latest: Self::Latest) -> Self {
        AppVersioned::V3(latest)
    }

    fn unwrap_latest(self) -> Result<Self::Latest> {
        match self {
            AppVersioned::V3(app) => Ok(app),
            AppVersioned::V1(_) | AppVersioned::V2(_) => bail!("version not latest"),
        }
    }

    fn decode_version(version: u16, body: Bytes) -> Result<Self, DecodeError> {
        match version {
            1 => Ok(AppVersioned::V1(schemas::v1::App::decode(body)?)),
            2 => Ok(AppVersioned::V2(schemas::v2::App::decode(body)?)),
            3 => Ok(AppVersioned::V3(schemas::v3::App::decode(body)?)),
            _ => Err(DecodeError::new(DecodeErrorKind::UnsupportedVersion, 0)),
        }
    }

    fn encode_version<S: Sink>(&self, _version: u16, out: &mut S) -> Result<(), EncodeError> {
        match self {
            AppVersioned::V1(app) => app.encode(out),
            AppVersioned::V2(app) => app.encode(out),
            AppVersioned::V3(app) => app.encode(out),
        }
        Ok(())
    }
}

impl AppVersioned {
    fn v1_to_v2(self) -> Result<Self> {
        use schemas::v2;
        match self {
            AppVersioned::V1(app) => {
                let mut todos = BTreeMap::new();
                for t in app.todos {
                    let id: v2::TodoId = u64::from(t.id);
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
                Ok(AppVersioned::V2(v2::App {
                    todos,
                    settings: BTreeMap::new(),
                }))
            }
            other => Ok(other),
        }
    }

    fn v2_to_v3(self) -> Result<Self> {
        use schemas::{v2, v3};
        match self {
            AppVersioned::V2(app) => {
                // Convert tags: list<str> -> map<TagId><Tag>
                fn convert_tags(tags: Vec<vbare::Str>) -> BTreeMap<v3::TagId, v3::Tag> {
                    let mut map = BTreeMap::new();
                    for (id, name) in (1..).zip(tags) {
                        map.insert(
                            id,
                            v3::Tag {
                                id,
                                name,
                                color: None,
                            },
                        );
                    }
                    map
                }

                let mut todos = BTreeMap::new();
                for (id, t) in app.todos {
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
                        detail: v3::TodoDetail {
                            title: t.title,
                            tags: convert_tags(t.tags),
                        },
                        history: Vec::new(),
                    };
                    todos.insert(id, todo);
                }

                Ok(AppVersioned::V3(v3::App {
                    todos,
                    config: v3::AppConfig {
                        theme: v3::Theme::System,
                        features: BTreeMap::new(),
                    },
                    boards: BTreeMap::new(),
                }))
            }
            other => Ok(other),
        }
    }

    fn v3_to_v2(self) -> Result<Self> {
        use schemas::{v2, v3};
        match self {
            AppVersioned::V3(app) => {
                let mut todos = BTreeMap::new();
                for (id, t) in app.todos {
                    let status = match t.status {
                        v3::TodoStatus::Open => v2::TodoStatus::Open,
                        v3::TodoStatus::InProgress => v2::TodoStatus::InProgress,
                        v3::TodoStatus::Done => v2::TodoStatus::Done,
                    };
                    let tags = t.detail.tags.into_values().map(|tag| tag.name).collect();
                    todos.insert(
                        id,
                        v2::Todo {
                            id,
                            title: t.detail.title,
                            status,
                            created_at: t.created_at,
                            tags,
                        },
                    );
                }
                Ok(AppVersioned::V2(v2::App {
                    todos,
                    settings: BTreeMap::new(),
                }))
            }
            other => Ok(other),
        }
    }

    fn v2_to_v1(self) -> Result<Self> {
        use schemas::{v1, v2};
        match self {
            AppVersioned::V2(app) => {
                let mut todos = Vec::new();
                for t in app.todos.into_values() {
                    todos.push(v1::Todo {
                        id: u32::try_from(t.id)?,
                        title: t.title,
                        done: matches!(t.status, v2::TodoStatus::Done),
                    });
                }
                Ok(AppVersioned::V1(v1::App { todos }))
            }
            other => Ok(other),
        }
    }
}

// Convenience function for consumers/tests
pub fn migrate_to_latest(payload: Bytes, version: u16) -> Result<schemas::v3::App, DecodeError> {
    AppVersioned::decode_body(payload, version)
}
