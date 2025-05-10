use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    #[serde(rename = "type")]
    pub type_: String,
    pub value: Option<String>,
}
