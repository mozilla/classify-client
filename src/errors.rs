use actix_web::HttpResponse;
use maxminddb::{self, MaxMindDBError};
use serde_derive::Serialize;
use std::fmt;

#[derive(Debug, Eq, PartialEq, Serialize)]
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
            MaxMindDBError::AddressNotFoundError(msg) => Self {
                message: format!("AddressNotFound: {}", msg),
            },
            MaxMindDBError::InvalidDatabaseError(msg) => Self {
                message: format!("InvalidDatabaseError: {}", msg),
            },
            MaxMindDBError::IoError(msg) => Self {
                message: format!("IoError: {}", msg),
            },
            MaxMindDBError::MapError(msg) => Self {
                message: format!("MapError: {}", msg),
            },
            MaxMindDBError::DecodingError(msg) => Self {
                message: format!("DecodingError: {}", msg),
            },
        }
    }
}

macro_rules! impl_from_error {
    ($error: ty) => {
        impl From<$error> for ClassifyError {
            fn from(error: $error) -> Self {
                Self::from_source(stringify!($error), error)
            }
        }
    };
}

impl_from_error!(actix_web::http::header::ToStrError);
impl_from_error!(config::ConfigError);
impl_from_error!(std::net::AddrParseError);
impl_from_error!(std::io::Error);
impl_from_error!(ipnet::AddrParseError);
