use std::{
    convert::Infallible,
    error::Error,
    fmt::{Display, Formatter},
};

pub use self::types::{FromType, Type};

mod types;

#[derive(Debug, Clone, Copy)]
pub struct Signature {
    pub cmd: &'static str,
    pub params: &'static [(&'static str, Type)],
}

impl Signature {
    fn get(&self, index: usize) -> Option<(&'static str, Type)> {
        self.params.get(index).copied()
    }

    pub fn parse_cmd(input: &str) -> Option<(&str, &str)> {
        if input.is_empty() {
            return None;
        }

        match input.split_once(' ') {
            Some((param, input)) => Some((param, input)),
            None => Some((input, "")),
        }
    }

    pub fn parser(self, input: &str) -> SignatureParser {
        SignatureParser::new(self, input)
    }
}

impl Display for Signature {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.cmd)?;

        let mut params = self.params.iter();
        loop {
            match params.next() {
                Some((_, Type::Opt)) => break,
                Some((name, ty)) => {
                    write!(f, " <{name}: {ty}>")?;
                }
                None => break,
            }
        }

        loop {
            match params.next() {
                Some((_, Type::Opt)) => return Err(std::fmt::Error),
                Some((name, ty)) => {
                    write!(f, " [{name}: {ty}]")?;
                }
                None => break,
            }
        }

        Ok(())
    }
}

pub struct SignatureParser<'a> {
    signature: Signature,
    is_required: bool,
    index: usize,
    input: &'a str,
}

impl<'a> SignatureParser<'a> {
    pub fn new(signature: Signature, input: &'a str) -> Self {
        Self {
            signature,
            is_required: true,
            index: 0,
            input,
        }
    }

    fn split_param(&mut self) -> Option<&'a str> {
        if self.input.is_empty() {
            return None;
        }

        match self.input.split_once(' ') {
            Some((param, input)) => {
                self.input = input;
                Some(param)
            }
            None => {
                let param = self.input;
                self.input = "";
                Some(param)
            }
        }
    }

    fn err<E>(&self, accessed: Option<(Type, bool)>, err: Option<E>) -> SignatureError<E>
    where
        E: Error,
    {
        SignatureError {
            expected: self.signature,
            index: self.index,
            is_required: self.is_required,
            accessed,
            err,
        }
    }

    pub fn parse_param<T, E>(&mut self) -> Result<T, SignatureError<E>>
    where
        T: FromType<'a, Error = E>,
        E: Error,
    {
        let ty = loop {
            let Some((_, ty)) = self.signature.get(self.index) else {
                return Err(self.err(Some(T::TYPE), None));
            };
            if let Type::Opt = ty {
                self.is_required = false;
                self.index += 1;
            } else {
                break ty;
            }
        };

        if (ty, self.is_required) != T::TYPE {
            return Err(self.err(Some(T::TYPE), None));
        }

        match (self.split_param(), self.is_required) {
            (Some(param), _) => {
                let out = T::from_type(param).map_err(|err| self.err(Some(T::TYPE), Some(err)));
                self.index += 1;
                out
            }
            (None, true) => Err(self.err(Some(T::TYPE), None)),
            (None, false) => {
                self.index += 1;
                Ok(T::default())
            }
        }
    }

    pub fn finish(&self) -> Result<(), SignatureError<Infallible>> {
        if self.signature.get(self.index).is_some() || !self.input.is_empty() {
            return Err(self.err(None, None));
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct SignatureError<E: Error> {
    /// expected signature
    expected: Signature,
    /// index of the argument being parsed right now
    index: usize,
    /// whether the current param is required
    is_required: bool,
    /// type that is trying to be accessed, if [`None`], `input` was expected to be empty
    accessed: Option<(Type, bool)>,
    /// parsing error of the param or something similar
    err: Option<E>,
}

impl<E: Error> Display for SignatureError<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.expected)?;

        let expected = self
            .expected
            .get(self.index)
            .map(|(name, ty)| (name, (ty, self.is_required)));
        let accessed = self.accessed;

        if let (None, Some((ty, required))) = (expected, accessed) {
            writeln!(
                f,
                "tried to access {} field of type `{ty}`, expected to be done with parsing input",
                {
                    if required {
                        "required"
                    } else {
                        "optional"
                    }
                }
            )
        } else if let (Some((name, expected)), Some(accessed)) = (expected, accessed) {
            write!(f, "{name}: ")?;
            if expected != accessed {
                writeln!(
                    f,
                    "failed to access type `{accessed}`, expected `{expected}`",
                    expected = expected.0,
                    accessed = accessed.0,
                )
            } else if let Some(err) = &self.err {
                writeln!(
                    f,
                    r#"failed parsing type `{accessed}` with "{err}"#,
                    accessed = accessed.0
                )
            } else {
                writeln!(
                    f,
                    "missing input for required argument of type `{expected}`",
                    expected = expected.0
                )
            }
        } else if let (Some((name, (expected, required))), None) = (expected, accessed) {
            writeln!(
                f,
                "tries to stop parsing, but signature is expecting {} field `{name}` of type `{expected}`",
                    if required {
                        "required"
                    } else {
                        "optional"
                    }
            )
        } else {
            writeln!(
                f,
                "expected to be done with parsing, but still has remaining input"
            )
        }
    }
}

impl<E: Error> Error for SignatureError<E> {}
