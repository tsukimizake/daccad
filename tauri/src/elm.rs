use elm_rs::{Elm, ElmDecode, ElmEncode};
use serde::Deserialize;
use serde::Serialize;

use crate::lisp::parser::Value;

#[derive(Serialize, Deserialize, Debug, Elm, ElmEncode, ElmDecode, Clone)]
#[serde(tag = "t", content = "c")]
pub enum ToTauriCmdType {
    // RequestStlFile(String),
    RequestCode(String),
    RequestEval,
}

#[derive(Serialize, Deserialize, Debug, Elm, ElmEncode, ElmDecode, Clone)]
#[serde(tag = "t", content = "c")]
pub enum FromTauriCmdType {
    // StlBytes(Vec<u8>),
    Code(String),
    EvalOk(Value),
    EvalError(String),
}
