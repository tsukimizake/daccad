use elm_rs::{Elm, ElmDecode, ElmEncode};
use serde::{Deserialize, Serialize};
use tsify::Tsify;

use crate::ModelId;

#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
pub struct Env {}

#[derive(Debug, Clone, Serialize, Deserialize, Elm, ElmDecode, ElmEncode, Tsify)]
pub struct Evaled {
    pub value: Value,
    pub env: Vec<u8>, // TMP
}

impl Evaled {
    pub fn new(value: Value, env: Vec<u8>) -> Self {
        Evaled { value, env }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Elm, ElmDecode, ElmEncode, Tsify)]
pub enum Value {
    Number(f64),
    String(String),
    Bool(bool),
    Model(ModelId),
    // Add other value types as needed
}
