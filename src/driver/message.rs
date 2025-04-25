#[derive(Clone, Debug)]
pub struct Message {
    pub message_type: String,
    pub data: Vec<String>,
}

impl Message {
    pub fn new(message_type: String, data: Vec<String>) -> Self {
        Self { message_type, data }
    }

    pub fn to_string(&self) -> String {
        format!(
            "{}{}{}",
            self.message_type.len(),
            self.message_type,
            self.data.join(" ")
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        self.to_string().as_bytes().to_vec()
    }

    pub fn decode(data: String) -> Option<Self> {
        let type_len = data.chars().next()?.to_digit(10)? as usize;
        let message_type = data.chars().skip(1).take(type_len).collect::<String>();

        let data_str = data.chars().skip(1 + type_len).collect::<String>();
        let data = data_str
            .split_whitespace()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();

        Some(Self::new(message_type, data))
    }
}
