use thiserror::Error;

use crate::game::PlayerId;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GameError {
    #[error("Player: {0} cannot move there")]
    CannotMoveHere(PlayerId),
    #[error("Cell not found)")]
    NotFoundCell,
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Player is dead")]
    PlayerIsDead,
    #[error("Invalid Animation was called")]
    InvalidAnimation,
    #[error("Error generating ID")]
    GenerateIdError,
    #[error("Entity is stunned")]
    IsStunned,
}
