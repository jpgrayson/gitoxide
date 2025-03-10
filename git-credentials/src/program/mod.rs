use std::process::{Command, Stdio};

use bstr::{BString, ByteSlice, ByteVec};

use crate::{helper, Program};

/// The kind of helper program to use.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Kind {
    /// The built-in `git credential` helper program, part of any `git` distribution.
    Builtin,
    /// A custom credentials helper, as identified just by the name with optional arguments
    ExternalName {
        /// The name like `foo` along with optional args, like `foo --arg --bar="a b"`, with arguments using `sh` shell quoting rules.
        /// The program executed will be `git-credential-foo` if `name_and_args` starts with `foo`.
        name_and_args: BString,
    },
    /// A custom credentials helper, as identified just by the absolute path to the program and optional arguments. The program is executed through a shell.
    ExternalPath {
        /// The absolute path to the executable, like `/path/to/exe` along with optional args, like `/path/to/exe --arg --bar="a b"`, with arguments using `sh`
        /// shell quoting rules.
        path_and_args: BString,
    },
    /// A script to execute with `sh`.
    ExternalShellScript(BString),
}

/// Initialization
impl Program {
    /// Create a new program of the given `kind`.
    pub fn from_kind(kind: Kind) -> Self {
        Program {
            kind,
            child: None,
            stderr: true,
        }
    }

    /// Parse the given input as per the custom helper definition, supporting `!<script>`, `name` and `/absolute/name`, the latter two
    /// also support arguments which are ignored here.
    pub fn from_custom_definition(input: impl Into<BString>) -> Self {
        let mut input = input.into();
        let kind = if input.starts_with(b"!") {
            input.remove(0);
            Kind::ExternalShellScript(input)
        } else {
            let path = git_path::from_bstr(
                input
                    .find_byte(b' ')
                    .map_or(input.as_slice(), |pos| &input[..pos])
                    .as_bstr(),
            );
            if git_path::is_absolute(path) {
                Kind::ExternalPath { path_and_args: input }
            } else {
                input.insert_str(0, "git credential-");
                Kind::ExternalName { name_and_args: input }
            }
        };
        Program {
            kind,
            child: None,
            stderr: true,
        }
    }
}

/// Builder
impl Program {
    /// By default `stderr` of programs is inherited and typically displayed in the terminal.
    pub fn suppress_stderr(mut self) -> Self {
        self.stderr = false;
        self
    }
}

impl Program {
    pub(crate) fn start(
        &mut self,
        action: &helper::Action,
    ) -> std::io::Result<(std::process::ChildStdin, Option<std::process::ChildStdout>)> {
        assert!(self.child.is_none(), "BUG: must not call `start()` twice");
        let mut cmd = match &self.kind {
            Kind::Builtin => {
                let mut cmd = Command::new(cfg!(windows).then(|| "git.exe").unwrap_or("git"));
                cmd.arg("credential").arg(action.as_arg(false));
                cmd
            }
            Kind::ExternalShellScript(for_shell)
            | Kind::ExternalName {
                name_and_args: for_shell,
            }
            | Kind::ExternalPath {
                path_and_args: for_shell,
            } => git_command::prepare(git_path::from_bstr(for_shell.as_bstr()).as_ref())
                .with_shell()
                .arg(action.as_arg(true))
                .into(),
        };
        cmd.stdin(Stdio::piped())
            .stdout(if action.expects_output() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stderr(if self.stderr { Stdio::inherit() } else { Stdio::null() });
        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().expect("stdin to be configured");
        let stdout = child.stdout.take();

        self.child = child.into();
        Ok((stdin, stdout))
    }

    pub(crate) fn finish(&mut self) -> std::io::Result<()> {
        let mut child = self.child.take().expect("Call `start()` before calling finish()");
        let status = child.wait()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Credentials helper program failed with status code {:?}", status.code()),
            ))
        }
    }
}

///
pub mod main;
pub use main::function::main;
