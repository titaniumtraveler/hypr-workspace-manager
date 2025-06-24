use anyhow::{anyhow, Result};
use std::{
    env::VarError,
    fmt::{Display, Write},
    path::{Path, PathBuf},
};

pub struct PathBuilder {
    path: PathBuf,
}

impl PathBuilder {
    pub fn from_basepath<T: Display>(basepath: T) -> Self {
        // 107 is the max length of linux socket paths
        let mut path = String::with_capacity(107);
        path.write_fmt(format_args!("{basepath}"))
            .expect("writing to String never fails");
        let mut path: PathBuf = path.into();
        path.push("_");
        Self { path }
    }

    pub fn with_filename<P: AsRef<Path>>(&mut self, name: P) -> &Path {
        self.path.pop();
        self.path.push(name);
        &self.path
    }
}

impl PathBuilder {
    pub fn niri_basepath(name: impl Display) -> Result<PathBuf> {
        let socket_path = match std::env::var(niri_ipc::socket::SOCKET_PATH_ENV) {
            Ok(path) => path,
            Err(VarError::NotPresent) => {
                return Err(anyhow!(
                    "expected to be started in the context of a running niri instance",
                ));
            }
            Err(VarError::NotUnicode(var)) => {
                return Err(anyhow!(
                    "invalid niri socket path {var:?}, expected it to be unicode"
                ));
            }
        };

        Ok(format!("{socket_path}-{name}.sock").into())
    }
}
