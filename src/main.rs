use std::env;
use std::ffi::OsString;
use std::os::raw::c_int;
use std::os::unix::process::CommandExt;
use std::process::{self, Command};

const EXPECTED_PARENT_PID_ENV: &str = "PDEATHSIG_EXPECTED_PARENT_PID";
const PR_SET_PDEATHSIG: c_int = 1;
const SIGTERM: c_int = 15;
const EX_USAGE: i32 = 2;
const EX_CANNOT_EXECUTE: i32 = 126;
const EX_NOT_FOUND_OR_PRCTL_FAILED: i32 = 127;

type Pid = c_int;

unsafe extern "C" {
    fn prctl(option: c_int, ...) -> c_int;
    fn getppid() -> Pid;
    fn raise(signal: c_int) -> c_int;
}

fn main() {
    if let Err(error) = run() {
        eprintln!("pdeathsig: {error}");
        process::exit(error.exit_code());
    }
}

fn run() -> Result<(), Error> {
    let mut args = env::args_os().skip(1);

    let Some(program) = args.next() else {
        return Err(Error::Usage);
    };

    let command_args: Vec<OsString> = args.collect();
    let parent_pid = expected_parent_pid()?;

    set_parent_death_signal()?;
    ensure_parent_is_still_alive(parent_pid)?;

    let error = Command::new(program).args(command_args).exec();
    Err(Error::Exec(error))
}

fn expected_parent_pid() -> Result<Pid, Error> {
    match env::var(EXPECTED_PARENT_PID_ENV) {
        Ok(value) => value
            .parse::<Pid>()
            .map_err(|_| Error::InvalidExpectedParentPid(value)),
        Err(env::VarError::NotPresent) => Ok(get_parent_pid()),
        Err(env::VarError::NotUnicode(value)) => Err(Error::NonUnicodeExpectedParentPid(value)),
    }
}

fn set_parent_death_signal() -> Result<(), Error> {
    let rc = unsafe { prctl(PR_SET_PDEATHSIG, SIGTERM) };

    if rc == 0 {
        Ok(())
    } else {
        Err(Error::Prctl(std::io::Error::last_os_error()))
    }
}

fn ensure_parent_is_still_alive(expected_parent_pid: Pid) -> Result<(), Error> {
    let actual_parent_pid = get_parent_pid();

    if actual_parent_pid == expected_parent_pid {
        Ok(())
    } else {
        unsafe {
            raise(SIGTERM);
        }

        Err(Error::ParentChanged {
            expected: expected_parent_pid,
            actual: actual_parent_pid,
        })
    }
}

fn get_parent_pid() -> Pid {
    unsafe { getppid() }
}

#[derive(Debug)]
enum Error {
    Usage,
    InvalidExpectedParentPid(String),
    NonUnicodeExpectedParentPid(OsString),
    Prctl(std::io::Error),
    ParentChanged { expected: Pid, actual: Pid },
    Exec(std::io::Error),
}

impl Error {
    fn exit_code(&self) -> i32 {
        match self {
            Self::Usage
            | Self::InvalidExpectedParentPid(_)
            | Self::NonUnicodeExpectedParentPid(_) => EX_USAGE,
            Self::Prctl(_) | Self::ParentChanged { .. } => EX_NOT_FOUND_OR_PRCTL_FAILED,
            Self::Exec(error) if error.kind() == std::io::ErrorKind::NotFound => {
                EX_NOT_FOUND_OR_PRCTL_FAILED
            }
            Self::Exec(_) => EX_CANNOT_EXECUTE,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage => write!(formatter, "usage: pdeathsig <command> [args...]"),
            Self::InvalidExpectedParentPid(value) => write!(
                formatter,
                "{EXPECTED_PARENT_PID_ENV} must be a valid pid, got {value:?}"
            ),
            Self::NonUnicodeExpectedParentPid(value) => write!(
                formatter,
                "{EXPECTED_PARENT_PID_ENV} must be valid unicode, got {value:?}"
            ),
            Self::Prctl(error) => write!(
                formatter,
                "prctl(PR_SET_PDEATHSIG, SIGTERM) failed: {error}"
            ),
            Self::ParentChanged { expected, actual } => write!(
                formatter,
                "parent changed before exec: expected parent pid {expected}, actual parent pid {actual}"
            ),
            Self::Exec(error) => write!(formatter, "exec failed: {error}"),
        }
    }
}

impl std::error::Error for Error {}
