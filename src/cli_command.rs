use std::{
    ffi::{OsStr, OsString},
    fmt,
};

use tokio::process::Command;

#[derive(Clone, Default, Eq, PartialEq)]
pub struct CliCommand {
    program: OsString,
    args: Vec<OsString>,
}

impl CliCommand {
    pub fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
        }
    }

    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    pub fn arg_opt<A>(mut self, arg: Option<A>) -> Self
    where
        A: AsRef<OsStr>,
    {
        if let Some(arg) = arg {
            self.args.push(arg.as_ref().to_os_string());
        }
        self
    }

    pub fn args<I, A>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = A>,
        A: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_os_string()));
        self
    }

    pub fn argv(&self) -> impl Iterator<Item = &OsStr> {
        std::iter::once(self.program.as_os_str()).chain(self.args.iter().map(OsString::as_os_str))
    }

    pub fn into_tokio_command(self) -> Command {
        let mut command = Command::new(self.program);
        command.args(self.args);
        command
    }
}

impl fmt::Debug for CliCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (idx, part) in self.argv().enumerate() {
            if idx > 0 {
                f.write_str(" ")?;
            }
            f.write_str(&part.to_string_lossy())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod cli_command {
        use super::*;

        mod arg {
            use super::*;

            #[test]
            fn test_appends_arg() {
                let cmd = CliCommand::new("echo").arg("hello");
                assert_eq!(format!("{cmd:?}"), "echo hello");
            }
        }

        mod arg_opt {
            use super::*;

            #[test]
            fn test_skips_none() {
                let cmd = CliCommand::new("echo").arg_opt::<&str>(None).arg("hello");
                assert_eq!(format!("{cmd:?}"), "echo hello");
            }

            #[test]
            fn test_appends_some() {
                let cmd = CliCommand::new("echo").arg_opt(Some("--flag"));
                assert_eq!(format!("{cmd:?}"), "echo --flag");
            }
        }
    }
}
