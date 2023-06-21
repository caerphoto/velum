use std::{fmt, io};

pub struct ParseError {
    pub cause: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ParseError: {}", self.cause)
    }
}

// TODO: make this more useful or something, I dunno
impl fmt::Debug for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ParseError: {}", self.cause)
    }
}

impl From<io::Error> for ParseError {
    fn from(error: io::Error) -> Self {
        Self {
            cause: format!("IO error: {:?}", error.to_string()),
        }
    }
}

impl From<String> for ParseError {
    fn from(msg: String) -> Self {
        Self {
            cause: format!("ParseError: {msg}"),
        }
    }
}

impl std::error::Error for ParseError {}

pub type ParseResult<T> = Result<T, ParseError>;
