use std::process::{ExitStatus, Command};
use std::io;

use thiserror::Error;

const UNIX_PYRODACTYL_USER: &str = "pyrodactyl";

#[derive(Debug, Error)]
pub enum EnvSetupError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("{}", fmt_subproc_error(.name, *.status, .stderr, .stdout))]
    Subprocess {
        name: &'static str,
        status: ExitStatus,
        stderr: Vec<u8>,
        stdout: Vec<u8>,
    },
}

/// Sets the system up for running Docker containers.  
///
/// 1. Creates the pyrodactyl system user
pub fn setup() -> Result<(), EnvSetupError> {
    imp::setup()
}

fn fmt_subproc_error(name: &'static str, status: ExitStatus, stderr: &[u8], stdout: &[u8]) -> String {
    let line1 = format!("subprocess '{name}' failed with status {status}");
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);

    format!("{}\nstdout:\n{}\nstderr:\n{}", line1, stdout.as_ref(), stderr.as_ref())
}

#[cfg(unix)]
mod imp {
    use super::*;

    pub fn setup() -> Result<(), EnvSetupError> {
        // better use the command to create user; safer than manually locking and appending to passw
        let output = Command::new("useradd")
            .arg("--no-create-home")
            .arg("--system")
            .arg("--shell")
            .arg("/usr/sbin/nologin")
            .arg(UNIX_PYRODACTYL_USER)
            .output()?;

        // see useradd(8) for exit code documentation
        let code = output.status.code();

        match code {
            Some(0) => {
                tracing::info!("successfully created OS user '{UNIX_PYRODACTYL_USER}'");
            }
            Some(9) => {
                tracing::info!("OS user '{UNIX_PYRODACTYL_USER}' already exists");
            }
            _ => {
                return Err(EnvSetupError::Subprocess {
                    name: "useradd",
                    status: output.status,
                    stderr: output.stderr,
                    stdout: output.stdout,
                }); 
            },
        }

        Ok(())
    }
}

#[cfg(windows)]
mod imp {
    pub fn setup() -> Result<(), super::EnvSetupError> {
        compile_error!("Windows user setup is not currently implemented!");
    }
}

