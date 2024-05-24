use std::sync::atomic::{AtomicBool, Ordering};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs::{self, Permissions};
use std::io;

use thiserror::Error;
use uuid::Uuid;

pub const PYRODACTYL_USER: &str = "pyrodactyl";

#[derive(Debug, Error)]
pub enum EnvSetupError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("previously logged OS library error")]
    Os,
}

static DATA_ROOT_INIT: AtomicBool = AtomicBool::new(false);

pub struct MountsDir {
    pub path: PathBuf,
}

impl MountsDir {
    fn get() -> io::Result<Self> {
        let path = Os::DATA_ROOT.join("mounts");
        fs::create_dir_all(path)?;

        Ok(Self {
            path,
        })
    }

    pub fn mount_of(&self, uuid: Uuid) -> io::Result<PathBuf> {
        let path = self.path.join(format!("{}", uuid.as_hyphenated()));
        fs::create_dir_all(path)?;

        let mut perms = fs::metadata(path)?.permissions();
        Os::transform_permissions_useronly(&mut perms);

        std::fs::set_permissions(path, perms)?;
        Ok(path)
    }
}

#[derive(Clone, Copy)]
pub struct DataRoot {
    _check: (),
}

impl DataRoot {
    pub fn init() -> Result<(), EnvSetupError> {
        let d = DataRoot { _check: () };

        fs::create_dir_all(d.mounts()?.path)?;

        Os::set_data_root_permissions()?;

        DATA_ROOT_INIT.store(true, Ordering::SeqCst);

        Ok(())
    }

    pub fn get() -> Self {
        if !DATA_ROOT_INIT.load(Ordering::SeqCst) {
            panic!("data root isn't initialized");
        }

        DataRoot { _check: () }
    }

    pub fn mounts(self) -> io::Result<MountsDir> {
        MountsDir::get()
    }
}

/// OS-dependent data root folder. `/var/lib/alerion` on *nix.
pub const fn data_root() -> &'static Path {
    Os::DATA_ROOT
}

/// Returns the user input docker should use.  
///
/// This will be in the form of "gid:uid" on *nix.  
///
/// Windows implementation pending.
pub fn setup_user() -> Result<String, EnvSetupError> {
    Os::setup_user()
}

trait OsImpl {
    const DATA_ROOT: &'static Path;

    fn set_data_root_permissions() -> Result<(), EnvSetupError>;
    fn setup_user() -> Result<String, EnvSetupError>;
    fn transform_permissions_useronly(perms: &mut Permissions);
}

#[cfg(unix)]
type Os = unix::Unix;

#[cfg(windows)]
type Os = windows::Windows;

#[cfg(unix)]
mod unix {
    use super::*;
    use std::ffi::{CStr, CString};
    use std::mem::zeroed;
    use std::ptr::null_mut;
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;
    use libc::{sysconf, gid_t, uid_t, getpwnam_r, _SC_GETPW_R_SIZE_MAX, ERANGE, S_IRUSR, S_IWUSR, S_IXUSR, __errno_location};

    pub struct Unix;

    impl OsImpl for Unix {
        const DATA_ROOT: &'static Path = Path::new("/var/lib");

        fn set_data_root_permissions() -> Result<(), EnvSetupError> {
            let data_root = data_root();
            let cstr = CString::new(data_root.to_str().expect("valid utf8")).expect("no nulls");
            let result = unsafe { libc::chmod(cstr.as_ptr(), S_IRUSR | S_IWUSR | S_IXUSR) };

            if result != 0 {
                let errno = unsafe { *__errno_location() };
                tracing::error!("chmod error: errno = {errno}");
                return Err(EnvSetupError::Os);
            }

            Ok(())
        }

        fn setup_user() -> Result<String, EnvSetupError> {
            fn get_passwd_record(uname: &CStr) -> Result<Option<(uid_t, gid_t)>, i32> {
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

        fn transform_permissions_useronly(perms: &mut Permissions) {
            perms.set_mode(0o700);
        }
    }
}

#[cfg(windows)]
mod windows {
    pub struct Windows;

    impl OsImpl for Windows {
        compile_error!("Windows not yet implemented");
    }
}

