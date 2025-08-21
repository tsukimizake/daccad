use elm_rs::{Elm, ElmDecode, ElmEncode};
use serde::{Deserialize, Serialize};
use tsify::Tsify;

#[derive(Debug, Clone, Serialize, Deserialize, Elm, ElmDecode, ElmEncode, Tsify)]
pub struct ModelId(pub usize);

impl From<usize> for ModelId {
    fn from(id: usize) -> Self {
        ModelId(id)
    }
}

pub struct Model {
    pub id: ModelId,
    // Add other fields as needed
}
