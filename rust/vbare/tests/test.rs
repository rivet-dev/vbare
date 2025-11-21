use anyhow::*;
use serde::{Deserialize, Serialize};
use vbare::OwnedVersionedData;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TestDataV1 {
    id: u32,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TestDataV2 {
    id: u32,
    name: String,
    description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TestDataV3 {
    id: u32,
    name: String,
    description: String,
    tags: Vec<String>,
}

#[derive(Clone)]
enum TestData {
    V1(TestDataV1),
    V2(TestDataV2),
    V3(TestDataV3),
}

impl OwnedVersionedData for TestData {
    type Latest = TestDataV3;

    fn wrap_latest(latest: TestDataV3) -> Self {
        TestData::V3(latest)
    }

    fn unwrap_latest(self) -> Result<Self::Latest> {
        #[allow(irrefutable_let_patterns)]
        if let TestData::V3(data) = self {
            Ok(data)
        } else {
            bail!("version not latest");
        }
    }

    fn deserialize_version(payload: &[u8], version: u16) -> Result<Self> {
        match version {
            1 => Ok(TestData::V1(serde_bare::from_slice(payload)?)),
            2 => Ok(TestData::V2(serde_bare::from_slice(payload)?)),
            3 => Ok(TestData::V3(serde_bare::from_slice(payload)?)),
            _ => bail!("invalid version: {version}"),
        }
    }

    fn serialize_version(self, _version: u16) -> Result<Vec<u8>> {
        match self {
            TestData::V1(data) => serde_bare::to_vec(&data).map_err(Into::into),
            TestData::V2(data) => serde_bare::to_vec(&data).map_err(Into::into),
            TestData::V3(data) => serde_bare::to_vec(&data).map_err(Into::into),
        }
    }

    fn deserialize_converters() -> Vec<impl Fn(Self) -> Result<Self>> {
        vec![Self::v1_to_v2, Self::v2_to_v3]
    }

    fn serialize_converters() -> Vec<impl Fn(Self) -> Result<Self>> {
        vec![Self::v3_to_v2, Self::v2_to_v1]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TestDataNoConvertersV1 {
    id: u32,
    name: String,
}

#[derive(Clone)]
enum TestDataNoConverters {
    V1(TestDataNoConvertersV1),
}

impl OwnedVersionedData for TestDataNoConverters {
    type Latest = TestDataNoConvertersV1;

    fn wrap_latest(latest: TestDataNoConvertersV1) -> Self {
        TestDataNoConverters::V1(latest)
    }

    fn unwrap_latest(self) -> Result<Self::Latest> {
        #[allow(irrefutable_let_patterns)]
        if let TestDataNoConverters::V1(data) = self {
            Ok(data)
        } else {
            bail!("version not latest");
        }
    }

    fn deserialize_version(payload: &[u8], version: u16) -> Result<Self> {
        match version {
            1 => Ok(TestDataNoConverters::V1(serde_bare::from_slice(payload)?)),
            _ => bail!("invalid version: {version}"),
        }
    }

    fn serialize_version(self, _version: u16) -> Result<Vec<u8>> {
        match self {
            TestDataNoConverters::V1(data) => serde_bare::to_vec(&data).map_err(Into::into),
        }
    }
}

impl TestData {
    fn v1_to_v2(self) -> Result<Self> {
        match self {
            TestData::V1(v1) => Ok(TestData::V2(TestDataV2 {
                id: v1.id,
                name: v1.name,
                description: "default".to_string(),
            })),
            other => Ok(other),
        }
    }

    fn v2_to_v3(self) -> Result<Self> {
        match self {
            TestData::V2(v2) => Ok(TestData::V3(TestDataV3 {
                id: v2.id,
                name: v2.name,
                description: v2.description,
                tags: vec![],
            })),
            other => Ok(other),
        }
    }

    fn v3_to_v2(self) -> Result<Self> {
        match self {
            TestData::V3(v3) => Ok(TestData::V2(TestDataV2 {
                id: v3.id,
                name: v3.name,
                description: v3.description,
            })),
            other => Ok(other),
        }
    }

    fn v2_to_v1(self) -> Result<Self> {
        match self {
            TestData::V2(v2) => Ok(TestData::V1(TestDataV1 {
                id: v2.id,
                name: v2.name,
            })),
            other => Ok(other),
        }
    }
}

#[test]
fn test_v2_to_v1_to_v2() {
    let data = TestDataV2 {
        id: 456,
        name: "test".to_string(),
        description: "will be stripped".to_string(),
    };

    let payload = TestData::V2(data.clone()).serialize(1).unwrap();

    let deserialized = TestData::deserialize(&payload, 1).unwrap();
    assert_eq!(deserialized.id, 456);
    assert_eq!(deserialized.name, "test");
    assert_eq!(deserialized.description, "default");
}

#[test]
fn test_v2_to_v2() {
    let data = TestDataV2 {
        id: 456,
        name: "test".to_string(),
        description: "data".to_string(),
    };

    let payload = TestData::V2(data.clone()).serialize(2).unwrap();

    // V2 data deserialized with version 2 will be converted to V3 (latest)
    let deserialized = TestData::deserialize(&payload, 2).unwrap();
    assert_eq!(deserialized.id, 456);
    assert_eq!(deserialized.name, "test");
    assert_eq!(deserialized.description, "data");
    assert_eq!(deserialized.tags.len(), 0); // Tags default to empty vec
}

#[test]
fn test_unsupported_version() {
    assert!(TestData::deserialize(&[], 99).is_err());
}

#[test]
fn test_v3_to_v2() {
    // This is the critical test case: serializing from V3 to V2
    // With 3 versions and 2 serialize converters (V3->V2, V2->V1),
    // serializing to version 2 should apply 1 converter (V3->V2)
    let data = TestDataV3 {
        id: 789,
        name: "v3_test".to_string(),
        description: "test description".to_string(),
        tags: vec!["tag1".to_string(), "tag2".to_string()],
    };

    // Serialize V3 data to V2 format (should strip tags)
    let payload = TestData::V3(data.clone()).serialize(2).unwrap();

    // Deserialize as V2 and verify tags were stripped
    let deserialized = TestData::deserialize(&payload, 2).unwrap();
    assert_eq!(deserialized.id, 789);
    assert_eq!(deserialized.name, "v3_test");
    assert_eq!(deserialized.description, "test description");
    // Tags should not be present in V2
}

#[test]
fn test_v3_to_v1() {
    // Test serializing from V3 all the way down to V1
    // Should apply both converters: V3->V2, then V2->V1
    let data = TestDataV3 {
        id: 999,
        name: "v3_to_v1_test".to_string(),
        description: "should be stripped".to_string(),
        tags: vec!["will be removed".to_string()],
    };

    // Serialize V3 data to V1 format
    let payload = TestData::V3(data.clone()).serialize(1).unwrap();

    // Deserialize as V1 and verify both description and tags were stripped
    let deserialized = TestData::deserialize(&payload, 1).unwrap();
    assert_eq!(deserialized.id, 999);
    assert_eq!(deserialized.name, "v3_to_v1_test");
    assert_eq!(deserialized.description, "default");
    assert_eq!(deserialized.tags.len(), 0);
}

#[test]
fn test_v3_to_v3() {
    // Test that serializing V3 to V3 preserves all data
    let data = TestDataV3 {
        id: 123,
        name: "v3_same".to_string(),
        description: "preserved".to_string(),
        tags: vec!["keep".to_string()],
    };

    let payload = TestData::V3(data.clone()).serialize(3).unwrap();

    let deserialized = TestData::deserialize(&payload, 3).unwrap();
    assert_eq!(deserialized.id, 123);
    assert_eq!(deserialized.name, "v3_same");
    assert_eq!(deserialized.description, "preserved");
    assert_eq!(deserialized.tags, vec!["keep".to_string()]);
}

#[test]
fn test_serialize() {
    let data = TestData::V3(TestDataV3 {
        id: 456,
        name: "serialize_test".to_string(),
        description: "will be stripped".to_string(),
        tags: vec!["tag1".to_string()],
    });

    // Test serializing to V1 (should convert V3 -> V2 -> V1)
    let result = data.clone().serialize(1).unwrap();
    let deserialized: TestDataV1 = serde_bare::from_slice(&result).unwrap();
    assert_eq!(deserialized.id, 456);
    assert_eq!(deserialized.name, "serialize_test");

    // Test serializing to V2 (should convert V3 -> V2)
    let result = data.clone().serialize(2).unwrap();
    let deserialized: TestDataV2 = serde_bare::from_slice(&result).unwrap();
    assert_eq!(deserialized.id, 456);
    assert_eq!(deserialized.name, "serialize_test");
    assert_eq!(deserialized.description, "will be stripped");

    // Test serializing to V3 (no conversion)
    let result = data.serialize(3).unwrap();
    let deserialized: TestDataV3 = serde_bare::from_slice(&result).unwrap();
    assert_eq!(deserialized.id, 456);
    assert_eq!(deserialized.name, "serialize_test");
    assert_eq!(deserialized.description, "will be stripped");
    assert_eq!(deserialized.tags, vec!["tag1".to_string()]);
}

#[test]
fn test_embedded_v2_to_v1_to_v2() {
    let data = TestDataV2 {
        id: 456,
        name: "test".to_string(),
        description: "will be stripped".to_string(),
    };

    let payload = TestData::V2(data.clone())
        .serialize_with_embedded_version(1)
        .unwrap();

    // First 2 bytes should be the version (1 in little-endian)
    assert_eq!(payload[0], 1u8);
    assert_eq!(payload[1], 0u8);

    // Deserializing V1 data converts it to V3 (latest)
    let deserialized = TestData::deserialize_with_embedded_version(&payload).unwrap();
    assert_eq!(deserialized.id, 456);
    assert_eq!(deserialized.name, "test");
    assert_eq!(deserialized.description, "default");
    assert_eq!(deserialized.tags.len(), 0);
}

#[test]
fn test_embedded_v2_to_v2() {
    let data = TestDataV2 {
        id: 456,
        name: "test".to_string(),
        description: "data".to_string(),
    };

    let payload = TestData::V2(data.clone())
        .serialize_with_embedded_version(2)
        .unwrap();

    // First 2 bytes should be the version (2 in little-endian)
    assert_eq!(payload[0], 2u8);
    assert_eq!(payload[1], 0u8);

    // Deserializing V2 data converts it to V3 (latest)
    let deserialized = TestData::deserialize_with_embedded_version(&payload).unwrap();
    assert_eq!(deserialized.id, 456);
    assert_eq!(deserialized.name, "test");
    assert_eq!(deserialized.description, "data");
    assert_eq!(deserialized.tags.len(), 0);
}

#[test]
fn test_no_converters() {
    let data = TestDataNoConvertersV1 {
        id: 456,
        name: "test".to_string(),
    };

    let payload = TestDataNoConverters::V1(data.clone()).serialize(1).unwrap();

    let deserialized = TestDataNoConverters::deserialize(&payload, 1).unwrap();
    assert_eq!(deserialized.id, 456);
    assert_eq!(deserialized.name, "test");
}
