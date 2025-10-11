use serde::{Serialize, Serializer};
use serde_json::Value;

#[derive(Debug, Clone)]
pub enum Data {
    None,
    Some(Value),
}

impl Serialize for Data {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Data::None => serializer.serialize_none(),
            Data::Some(value) => serializer.serialize_some(value),
        }
    }
}

