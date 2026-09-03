//! Frames that land in and leave from [`classified`] containers (feature
//! `classified`).
//!
//! A received secret is moved into a [`ClassifiedBuffer`] bounded to its own
//! length, so the only plain copy is the one the reader filled, and that
//! one becomes the container. A sent secret is exposed for the duration of
//! the write and nowhere else.

use std::io::{Read, Write};

use abut::{AbutError, FramedReader, FramedWriter};
use classified::{ClassifiedBuffer, ClassifiedError};
use liaise::{Liaise, LiaiseCodes};

/// Why a classified receive failed. Renders as a `MINE` code; the frame or
/// container error underneath is the [`source`](std::error::Error::source).
#[derive(LiaiseCodes, Debug)]
#[liaise(prefix = "MINE")]
pub enum RecvError {
    /// The frame could not be read.
    #[liaise(code = 10, msg = "Frame could not be received", source)]
    Frame(AbutError),
    /// The frame was empty; a container refuses an empty secret.
    #[liaise(code = 11, msg = "Empty frame refused by the container", source)]
    Empty(ClassifiedError),
}

/// Receive one frame into a zeroizing buffer bounded to its length.
pub fn recv_classified<R: Read>(reader: &mut FramedReader<R>) -> Result<ClassifiedBuffer, RecvError> {
    let mut bytes = Vec::new();
    reader.recv_into(&mut bytes)?;
    let len = bytes.len();
    Ok(ClassifiedBuffer::try_from_vec(bytes, len)?)
}

/// Send the contents of a container as one frame.
pub fn send_classified<W: Write>(writer: &mut FramedWriter<W>, secret: &ClassifiedBuffer) -> Result<(), AbutError> {
    secret.expose(|view| writer.write_frame(view.as_bytes()))
}
