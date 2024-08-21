use std::{
    convert::Infallible,
    error::Error,
    fmt::{Display, Formatter},
    num::ParseIntError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Str,
    U8,
    Opt,
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Str => f.write_str("&str"),
            Type::U8 => f.write_str("u8"),
            Type::Opt => f.write_str(""),
        }
    }
}

pub trait FromType<'a>: Default {
    const TYPE: (Type, bool);
    type Error: Error;

    fn from_type(param: &'a str) -> Result<Self, Self::Error>
    where
        Self: Sized;
}

impl<'a> FromType<'a> for &'a str {
    const TYPE: (Type, bool) = (Type::Str, true);

    type Error = Infallible;
    fn from_type(param: &'a str) -> Result<Self, Self::Error> {
        Ok(param)
    }
}

impl<'a> FromType<'a> for Option<&'a str> {
    const TYPE: (Type, bool) = (Type::Str, false);

    type Error = Infallible;
    fn from_type(param: &'a str) -> Result<Self, Self::Error> {
        Ok(Some(param))
    }
}

impl FromType<'_> for u8 {
    const TYPE: (Type, bool) = (Type::U8, true);

    type Error = ParseIntError;
    fn from_type(param: &str) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        param.parse()
    }
}

impl FromType<'_> for Option<u8> {
    const TYPE: (Type, bool) = (Type::U8, false);

    type Error = ParseIntError;
    fn from_type(param: &str) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        param.parse().map(Some)
    }
}
