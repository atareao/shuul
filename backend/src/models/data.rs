//! # Tipo de datos genérico para respuestas
//!
//! [`Data`] representa el campo `data` de las respuestas de la API.
//! Puede contener un valor JSON arbitrario o ser vacío (`None`).

use serde::{Serialize, Serializer};
use serde_json::Value;

/// Contenedor de datos para respuestas de la API.
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
            Self::None => serializer.serialize_none(),
            Self::Some(value) => serializer.serialize_some(value),
        }
    }
}
