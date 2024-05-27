use std::ffi::{CStr, CString};
use std::mem::zeroed;
use std::env;
use std::path::{PathBuf, Path};
use std::process::Command;
use std::ptr::null_mut;

use libc::{__errno_location, getpwnam_r, gid_t, sysconf, uid_t, ERANGE, _SC_GETPW_R_SIZE_MAX};
use thiserror::Error;

use super::PYRODACTYL_USER;

#[derive(Debug, Error)]
#[error("libc error: errno={errno} '{ctx}'")]
pub struct LibcError {
    errno: i32,
    ctx: String,
}

impl LibcError {
    fn with_ctx(ctx: String) -> LibcError {
        LibcError {
            errno: get_errno(),
            ctx,
        }
    }
}

pub struct User {
    uid: u32,
    gid: u32,
}

impl super::UserImpl for User {
    fn ensure_exists() -> Result<Self, super::OsError> {
        let user_cstr = CString::new(PYRODACTYL_USER).expect("no null byte in PYRODACTYL_USER");
        let maybe_record = get_passwd_record(&user_cstr)?;

        let record = match maybe_record {
            Some(r) => r,
            None => {
                // the lazy solution (it's safer anyways)
                let output = Command::new("/usr/sbin/useradd")
                    .arg("--no-create-home")
                    .arg("--system")
                    .arg("--shell")
                    .arg("/usr/sbin/nologin")
                    .arg(PYRODACTYL_USER)
                    .output()?;

                // see useradd(8) for exit code documentation
                let code = output.status.code();

                match code {
                    Some(0) => {
                        tracing::info!("successfully created OS user '{PYRODACTYL_USER}'");
                    }
                    Some(9) => {
                        tracing::error!("conflict: OS user '{PYRODACTYL_USER}' already exists while getpwnam_r previously couldn't find an appropriate record");
                        tracing::error!("attempting to read record again");
                    }
                    maybe_code => {
                        tracing::error!("useradd failed with error code {maybe_code:#?}");
                        return Err(super::OsError::Other);
                    }
                }

                let Some(record) = get_passwd_record(&user_cstr)? else {
                    tracing::error!("couldn't find passwd record for user {PYRODACTYL_USER} despite it being created by `/usr/sbin/useradd` previously.");
                    return Err(super::OsError::Other);
                };

                record
            }
        };

        Ok(User {
            uid: record.0,
            gid: record.1,
        })
    }

    fn host_uname(&self) -> Result<String, super::OsError> {
        Ok(format!("{}:{}", self.uid, self.gid))
    }
}

pub struct ConfigPath;

impl super::ConfigPathImpl for ConfigPath {
    fn parent() -> Result<PathBuf, (env::VarError, &'static str)> {
        const VAR: &str = "XDG_CONFIG_HOME";
        let value = env::var(VAR).map_err(|e| (e, VAR))?;
        let path = Path::new(&value);
        Ok(path.join("alerion"))
    }

    fn node() -> &'static str {
        "config.json"
    }
}

fn get_passwd_record(uname: &CStr) -> Result<Option<(uid_t, gid_t)>, LibcError> {
    unsafe {
        let mut passwd = zeroed();
        let mut result = null_mut();

        let mut sizeguess = sysconf(_SC_GETPW_R_SIZE_MAX);
        if sizeguess == -1 {
            tracing::debug!("failed to sizeguess POSIX_GETPW_R_SIZE_MAX; fallback");
            sizeguess = 2048;
        }

        let mut bufsize = sizeguess as usize;
        let mut buf = Vec::with_capacity(bufsize);

        for _ in 0..5 {
            let r = getpwnam_r(
                uname.as_ptr(),
                &mut passwd,
                buf.as_mut_ptr(),
                bufsize,
                &mut result,
            );

            if result.is_null() {
                if r == ERANGE {
                    // range error, try again
                    const BUF_TRY_LIMIT: usize = 2 ^ 16;

                    bufsize = usize::min(bufsize * 2, BUF_TRY_LIMIT);
                    buf.resize(bufsize, 0);

                    continue;
                } else if r == 0 {
                    // no record found
                    return Ok(None);
                } else {
                    let ctx = format!("INTERNAL ERROR (PLEASE REPORT): getpwnam_r returned {r}");
                    return Err(LibcError::with_ctx(ctx));
                }
            }

            break;
        }

        if result.is_null() {
            let ctx = "INTERNAL ERROR (PLEASE REPORT): libc getpwnam_r keeps failing; can't get uid/gid";
            return Err(LibcError::with_ctx(ctx.to_owned()));
        }

        let gid = passwd.pw_gid;
        let uid = passwd.pw_uid;

        Ok(Some((uid, gid)))
    }
}

fn get_errno() -> i32 {
    let loc = unsafe { __errno_location() };
    if loc.is_null() {
        0
    } else {
        unsafe { *loc }
    }
}
