use std::{
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
