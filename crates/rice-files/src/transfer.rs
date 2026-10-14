//! Drag/drop Strategy translated into validated file-operation Commands.

use crate::{
    CommandId, EntryMetadata, GuestPath, OperationCommand, OperationError, OperationPlan,
    OperationScope,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropAction {
    Copy,
    Move,
    Link,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DropPlan {
    pub action: DropAction,
    pub operation: OperationPlan,
}

impl DropPlan {
    /// # Errors
    ///
    /// Returns validation errors from the selected file-operation Command.
    pub fn new(
        id: CommandId,
        scope: &OperationScope,
        sources: Vec<EntryMetadata>,
        target_directory: GuestPath,
        action: DropAction,
    ) -> Result<Self, OperationError> {
        let command = match action {
            DropAction::Copy => OperationCommand::copy(scope, sources, target_directory)?,
            DropAction::Move => OperationCommand::move_to(scope, sources, target_directory)?,
            DropAction::Link => OperationCommand::link(scope, sources, target_directory)?,
        };
        Ok(Self {
            action,
            operation: OperationPlan::new(id, command),
        })
    }
}
