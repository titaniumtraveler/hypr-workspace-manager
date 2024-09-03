use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Request<'a> {
    Create { name: &'a str },
    Bind { name: &'a str, register: u8 },
    Unbind { register: u8 },
    Goto { register: u8 },
    Moveto { register: u8 },
    Read { name: Option<&'a str> },
    Flush,
}
