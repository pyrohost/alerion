// TODO: fix :sob:

use std::borrow::Cow;
use std::fs::{self, Permissions};
use std::io;
use std::path::Path;

use thiserror::Error;

#[derive(Debug, Error)]
#[error("WinAPI error: {ctx}")]
pub struct WinapiError {
    ctx: String,
}

pub struct User;

impl super::UserImpl for User {
    fn ensure_exists() -> Result<Self, OsError> {
        unimplemented!()
    }

    fn host_uname(&self) -> Result<String, OsError> {
        unimplemented!()
    }
}

impl super::OsError for WinapiError {}

pub struct DataDirectory;

impl super::DataDirectoryImpl for DataDirectory {
    type OsError = WinapiError;

    fn path() -> Cow<'static, Path> {
        Cow::Borrowed(Path::new("%ProgramData%/Alerion/Lib"))
    }

    fn initialize() -> Result<(), super::EnvErrorImpl<Self::OsError>> {
        let path = Self::path();
        fs::create_dir_all(path)?;

        // TODO: Windows Permissions (??)

        Ok(())
    }

    fn mounts() -> super::Mounts {
        super::Mounts {
            path: Self::path().join("Mounts"),
        }
    }
}

pub struct ConfigFile;

impl super::ConfigFileImpl for ConfigFile {
    fn path() -> Cow<'static, Path> {
        Cow::Borrowed(Path::new("%ProgramData%/Alerion/Configuration.json"))
    }

    fn read() -> io::Result<String> {
        let contents = fs::read_to_string(Self::path())?;
        Ok(contents)
    }

    fn write(contents: &str) -> io::Result<()> {
        fs::write(Self::path(), contents)?;
        Ok(())
    }
}
