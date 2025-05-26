use std::{
    error::Error,
    fmt::{self, Display},
};

#[derive(Debug)]
pub enum FrameError {
    Io(std::io::Error),
    ParsingError(String),
    TypeCapacity(String),
    TypeConversionFailure(String),
}

impl Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::ParsingError(err) => {
                write!(f, "Parsing error : [{err:?}]")
            }
            FrameError::TypeCapacity(err) => {
                write!(f, "TypeCapacity error : [{err:?}]")
            }
            FrameError::TypeConversionFailure(err) => {
                write!(f, "TypeConversionFailure error : [{err:?}]")
            }
            FrameError::Io(e) => {
                write!(f, "Io error : [{e:?}]")
            }
        }
    }
}

impl Error for FrameError {}

impl From<std::io::Error> for FrameError {
    fn from(value: std::io::Error) -> Self {
        FrameError::Io(value)
    }
}
