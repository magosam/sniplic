use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Erro de I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("Erro de serialização JSON: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Operação inválida: {0}")]
    InvalidOperation(String),

    #[error("Erro no subsistema de mídia: {0}")]
    MediaError(String),

    #[error("Erro interno do core: {0}")]
    Internal(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;