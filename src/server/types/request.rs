use crate::server::State;
use serde::{Deserialize, Serialize};

pub use self::workspace::Workspace;

mod workspace;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Request<'a> {
    Create { name: &'a str },
    Delete { name: &'a str },
    Bind { name: &'a str, register: &'a str },
    Unbind { register: &'a str },
    GotoRegister { register: &'a str },
    MovetoRegister { register: &'a str, focus: bool },
    GotoName { name: &'a str },
    MovetoName { name: &'a str, focus: bool },
    Read { workspace: Option<Workspace<'a>> },
    Write(State),
    Flush,
}
