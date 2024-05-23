use std::process::Command;
use std::io;

use thiserror::Error;

pub const PYRODACTYL_USER: &str = "pyrodactyl";

#[derive(Debug, Error)]
pub enum EnvSetupError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("previously logged OS library error")]
    Os,
}

/// Returns the user input docker should use.
pub fn setup_user() -> Result<String, EnvSetupError> {
    imp::setup_user()
}

#[cfg(unix)]
mod imp {
    use super::*;
    use std::ffi::{CStr, CString};
    use std::mem::zeroed;
    use std::ptr::null_mut;
    use libc::{sysconf, gid_t, uid_t, getpwnam_r, _SC_GETPW_R_SIZE_MAX, ERANGE};

    pub fn fmt_os_lib_err(code: i32) -> String {
        format!("getpwdnam_r: returned {code}")
    }

    pub fn get_passwd_record(uname: &CStr) -> Result<Option<(uid_t, gid_t)>, i32> {
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
                        // other error
                        tracing::error!("INTERNAL ERROR (PLEASE REPORT): getpwnam_r: returned {r}");
                        return Err(r);
                    }
                }

                break;
            }

            if result.is_null() {
                tracing::debug!("INTERNAL ERROR (PLEASE REPORT): libc getpwnam_r keeps failing; can't get uid/gid");
                return Err(0);
            }

            let gid = passwd.pw_gid;
            let uid = passwd.pw_uid;

            Ok(Some((uid, gid)))
        }
    }

    pub fn setup_user() -> Result<String, EnvSetupError> {
        let user_cstr = CString::new(PYRODACTYL_USER).expect("no null byte in PYRODACTYL_USER");
        let maybe_record = get_passwd_record(&user_cstr).map_err(|_| EnvSetupError::Os)?;

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
                        return Err(EnvSetupError::Os); 
                    },
                }

                let Some(record) = get_passwd_record(&user_cstr).map_err(|_| EnvSetupError::Os)? else {
                    tracing::error!("couldn't find passwd record for user {PYRODACTYL_USER} despite it being created by `/usr/sbin/useradd` previously.");
                    return Err(EnvSetupError::Os);
                };

                record
            }
        };

        Ok(format!("{}:{}", record.0, record.1))
    }
}

#[cfg(windows)]
mod imp {
    pub fn setup() -> Result<(), super::EnvSetupError> {
        compile_error!("Windows user setup is not currently implemented!");
    }
}

