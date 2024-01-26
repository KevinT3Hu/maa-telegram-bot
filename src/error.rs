use std::{fmt::Display, sync::PoisonError};

use axum::{http::StatusCode, response::{IntoResponse, Response}};
use serde::Serialize;
use teloxide::RequestError;


#[derive(Debug, Serialize)]
#[allow(clippy::module_name_repetitions)]
pub enum AppError {

    DeviceNotFound(String),

    UserNotFound(String),

    PoisonError(String),

    TaskNotFound(String),

    TeloxideError(String),

    StateNotSet,
}

impl From<RequestError> for AppError {

    fn from(e: RequestError) -> Self {

        Self::TeloxideError(e.to_string())
    }
}

impl<T> From<PoisonError<T>> for AppError {

    fn from(e: PoisonError<T>) -> Self {

        Self::PoisonError(e.to_string())
    }
}

impl IntoResponse for AppError {

    fn into_response(self) -> Response {

        tracing::error!("AppError: {}", self);

        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

impl Display for AppError {

    #[allow(clippy::absolute_paths)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {

        match *self {
            AppError::DeviceNotFound(ref e) => write!(f, "Device not found with id: {e}"),
            AppError::UserNotFound(ref e) => write!(f, "User not found with id: {e}"),
            AppError::PoisonError(ref e) => write!(f, "PoisonError: {e}"),
            AppError::TaskNotFound(ref e) => write!(f, "Task not found with id: {e}"),
            AppError::TeloxideError(ref e) => write!(f, "TeloxideError: {e}"),
            AppError::StateNotSet => write!(f, "State not set"),
        }
    }
}

#[allow(clippy::absolute_paths)]
impl std::error::Error for AppError {}