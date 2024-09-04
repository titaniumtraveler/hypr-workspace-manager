use serde::{de::Visitor, Deserialize, Serialize};
use std::{
    fmt::{self, Debug, Display},
    marker::PhantomData,
};

#[derive(Debug, Clone, Copy)]
pub enum Workspace<'a> {
    Register(u8),
    Workspace(&'a str),
}

impl Serialize for Workspace<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Workspace::Register(register) => serializer.serialize_u8(*register),
            Workspace::Workspace(workspace) => serializer.serialize_str(workspace),
        }
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for Workspace<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(WorkspaceVisitor(PhantomData))
    }
}

struct WorkspaceVisitor<'a>(PhantomData<&'a ()>);

impl<'de: 'a, 'a> Visitor<'de> for WorkspaceVisitor<'a> {
    type Value = Workspace<'a>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a u8 or borrowed str")
    }

    fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Workspace::Register(v))
    }

    fn visit_borrowed_str<E>(self, v: &'a str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Workspace::Workspace(v))
    }
}

impl Display for Workspace<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Workspace::Register(register) => write!(f, "register:{register}"),
            Workspace::Workspace(workspace) => write!(f, "workspace:{workspace}"),
        }
    }
}
