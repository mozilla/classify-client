use actix_web::HttpResponse;
use maxminddb::{self, MaxMindDBError};
use serde_derive::Serialize;
use std::fmt;

#[derive(Debug, Serialize)]
pub struct ClassifyError {
    message: String,
}

impl ClassifyError {
    pub fn new<M: Into<String>>(message: M) -> Self {
        let message = message.into();
        Self { message }
    }

    pub fn from_source<S: fmt::Display, E: fmt::Display>(source: S, err: E) -> Self {
        Self {
            message: format!("{}: {}", source, err),
        }
    }
}

// Use default implementation of Error
impl std::error::Error for ClassifyError {}

impl fmt::Display for ClassifyError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{:?}", self)?;
        Ok(())
    }
}

impl actix_web::error::ResponseError for ClassifyError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::InternalServerError().json(self)
    }
}

impl From<MaxMindDBError> for ClassifyError {
    fn from(error: MaxMindDBError) -> Self {
        match error {
            MaxMindDBError::AddressNotFoundError(msg) => ClassifyError {
                message: format!("AddressNotFound: {}", msg),
            },
            MaxMindDBError::InvalidDatabaseError(msg) => ClassifyError {
                message: format!("InvalidDatabaseError: {}", msg),
            },
            MaxMindDBError::IoError(msg) => ClassifyError {
                message: format!("IoError: {}", msg),
            },
            MaxMindDBError::MapError(msg) => ClassifyError {
                message: format!("MapError: {}", msg),
            },
            MaxMindDBError::DecodingError(msg) => ClassifyError {
                message: format!("DecodingError: {}", msg),
            },
        }
    }
}

impl From<actix_web::http::header::ToStrError> for ClassifyError {
    fn from(error: actix_web::http::header::ToStrError) -> Self {
        Self::from_source("ToStrError", error)
    }
}

impl From<std::net::AddrParseError> for ClassifyError {
    fn from(error: std::net::AddrParseError) -> Self {
        Self::from_source("AddrParseError", error)
    }
}
