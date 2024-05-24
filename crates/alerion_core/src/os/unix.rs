use std::path::Path;
use std::borrow::Cow;
use std::fs::{self, Permissions};
use std::io;
use std::process::Command;
use std::os::unix::fs::PermissionsExt;
use std::ffi::{CStr, CString};
use std::mem::zeroed;
use std::ptr::null_mut;

use libc::{sysconf, gid_t, uid_t, getpwnam_r, _SC_GETPW_R_SIZE_MAX, ERANGE, __errno_location};
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
                    },
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
 
pub struct DataDirectory;

impl super::DataDirectoryImpl for DataDirectory {
    fn path() -> Cow<'static, Path> {
        Cow::Borrowed(Path::new("/var/lib/alerion"))
    }

    fn initialize() -> Result<(), super::OsError> {
        let path = Self::path();
        fs::create_dir_all(&path)?;
        
        let perms = Permissions::from_mode(0o700);
        fs::set_permissions(&path, perms)?;

        Ok(())
    }

    fn mounts() -> super::Mounts {
        super::Mounts { path: Self::path().join("mounts") }
    }
}

pub struct ConfigFile;

impl super::ConfigFileImpl for ConfigFile {
    fn path() -> Cow<'static, Path> {
        Cow::Borrowed(Path::new("/etc/alerion/config.json"))
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
            let r = getpwnam_r(uname.as_ptr(), &mut passwd, buf.as_mut_ptr(), bufsize, &mut result);

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
            let ctx = format!("INTERNAL ERROR (PLEASE REPORT): libc getpwnam_r keeps failing; can't get uid/gid");
            return Err(LibcError::with_ctx(ctx));
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
