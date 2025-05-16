use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    #[serde(rename = "type")]
    pub type_: String,
    pub value: Option<Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ButtonConfig {
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub icon: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub buttons: Option<HashMap<String, ButtonConfig>>,
    pub status: Option<ButtonConfig>,
}
