//! Frames that land in and leave from [`classified`] containers (feature
//! `classified`).
//!
//! A received secret is moved into a [`ClassifiedBuffer`] bounded to its own
//! length, so the only plain copy is the one the reader filled, and that
//! one becomes the container. A sent secret is exposed for the duration of
//! the write and nowhere else.

use std::fmt;
use std::io::{Read, Write};

use abut::{AbutError, FramedReader, FramedWriter};
use classified::{ClassifiedBuffer, ClassifiedError};

/// Why a classified receive failed.
#[derive(Debug)]
pub enum RecvError {
    /// The frame could not be read.
    Frame(AbutError),
    /// The frame was empty; a container refuses an empty secret.
    Empty(ClassifiedError),
}

impl fmt::Display for RecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame(e) => e.fmt(f),
            Self::Empty(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for RecvError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Frame(e) => Some(e),
            Self::Empty(e) => Some(e),
        }
    }
}

/// Receive one frame into a zeroizing buffer bounded to its length.
pub fn recv_classified<R: Read>(reader: &mut FramedReader<R>) -> Result<ClassifiedBuffer, RecvError> {
    let mut bytes = Vec::new();
    reader.recv_into(&mut bytes).map_err(RecvError::Frame)?;
    let len = bytes.len();
    ClassifiedBuffer::try_from_vec(bytes, len).map_err(RecvError::Empty)
}

/// Send the contents of a container as one frame.
pub fn send_classified<W: Write>(writer: &mut FramedWriter<W>, secret: &ClassifiedBuffer) -> Result<(), AbutError> {
    secret.expose(|view| writer.write_frame(view.as_bytes()))
}
