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
    pub fn hypr_basepath() -> Result<Self> {
        let instance = match std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
            Ok(instance) => instance,
            Err(VarError::NotPresent) => {
                return Err(anyhow!(
                    "expected to be started in the context of a running hyprland instance",
                ));
            }
            Err(VarError::NotUnicode(var)) => {
                return Err(anyhow!(
                    "invalid hyprland instance signature {var:?}, expected it to be unicode"
                ));
            }
        };

        Ok(PathBuilder::from_basepath(format_args!(
            "/run/user/1000/hypr/{instance}"
        )))
    }
}
