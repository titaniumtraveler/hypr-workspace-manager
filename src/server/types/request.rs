use serde::{Deserialize, Serialize};

pub use self::workspace::Workspace;

mod workspace;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Request<'a> {
    Create { name: &'a str },
    Bind { name: &'a str, register: u8 },
    Unbind { register: u8 },
    GotoRegister { register: u8 },
    MovetoRegister { register: u8 },
    GotoName { name: &'a str },
    MovetoName { name: &'a str },
    Read { workspace: Option<Workspace<'a>> },
    Flush,
}
