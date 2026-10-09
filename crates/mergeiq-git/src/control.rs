//! Continue / abort / skip for the operation in progress.

use crate::error::{GitError, Result};
use crate::exec::command_failed;
use crate::operation::Operation;
use crate::repo::Repo;

/// What happened after a control command.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ControlOutcome {
    /// The operation is no longer in progress.
    pub finished: bool,
    /// Combined git output, for display.
    pub message: String,
}

#[derive(Clone, Copy)]
enum Action {
    Continue,
    Abort,
    Skip,
}

impl Repo {
    /// Continues the operation. Refuses while unmerged paths remain.
    pub fn op_continue(&self) -> Result<ControlOutcome> {
        let operation = self.operation()?;
        let count = self.unmerged_count()?;
        if count > 0 {
            return Err(GitError::UnresolvedPaths { count });
        }
        self.run_control(&operation, Action::Continue)
    }

    /// Aborts the operation, restoring the pre-operation state.
    pub fn op_abort(&self) -> Result<ControlOutcome> {
        let operation = self.operation()?;
        self.run_control(&operation, Action::Abort)
    }

    /// Skips the current step. Rebase only.
    pub fn op_skip(&self) -> Result<ControlOutcome> {
        let operation = self.operation()?;
        self.run_control(&operation, Action::Skip)
    }

    fn run_control(&self, operation: &Operation, action: Action) -> Result<ControlOutcome> {
        let command = match operation {
            Operation::None => return Err(GitError::NoOperation),
            Operation::Unknown => {
                return Err(GitError::Unsupported {
                    what: "the conflicts have no operation git can continue or abort".into(),
                })
            }
            Operation::Merge => "merge",
            Operation::Rebase { .. } => "rebase",
            Operation::CherryPick => "cherry-pick",
            Operation::Revert => "revert",
            Operation::Am => "am",
        };
        let flag = match action {
            Action::Continue => "--continue",
            Action::Abort => "--abort",
            Action::Skip => {
                if !matches!(operation, Operation::Rebase { .. }) {
                    return Err(GitError::Unsupported {
                        what: "skip is only available during a rebase".into(),
                    });
                }
                "--skip"
            }
        };
        let args: Vec<std::ffi::OsString> = ["-c", "core.editor=true", command, flag]
            .map(Into::into)
            .to_vec();
        self.note_mutation();
        let output = self.exec.run(&self.root, &args)?;
        self.note_mutation();
        let message = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .trim()
        .to_string();
        if output.status.success() {
            return Ok(ControlOutcome {
                finished: self.operation()? == Operation::None,
                message,
            });
        }
        // `rebase --continue/--skip` exits non-zero when it stops at the next conflict.
        if !matches!(action, Action::Abort) && self.unmerged_count()? > 0 {
            return Ok(ControlOutcome {
                finished: false,
                message,
            });
        }
        Err(command_failed(&args, &output))
    }
}
