//! Client session management
//!
//! This module provides the Client struct which represents an individual
//! client connection and manages its state.

use crate::connection::Connection;
use crate::protocol::{ByteOrder, X11Error};
use std::io;

/// Represents a connected X11 client
pub struct Client {
    /// Unique client ID assigned by the resource tracker
    pub client_id: u32,

    /// The network connection to this client
    connection: Connection,

    /// Byte order for this client (from setup request)
    byte_order: ByteOrder,

    /// Sequence number for requests (for debugging/logging)
    sequence_number: u32,
}

impl Client {
    /// Create a new client session
    pub fn new(client_id: u32, connection: Connection, byte_order: ByteOrder) -> Self {
        Client {
            client_id,
            connection,
            byte_order,
            sequence_number: 0,
        }
    }

    /// Get the client's byte order
    pub fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    /// Get the current sequence number
    pub fn sequence_number(&self) -> u32 {
        self.sequence_number
    }

    /// Read data from the client connection
    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.connection.read(buf)
    }

    /// Write data to the client connection
    pub fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.connection.write(buf)
    }

    /// Write all data to the client connection
    pub fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        let mut written = 0;
        while written < buf.len() {
            match self.connection.write(&buf[written..]) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to write whole buffer",
                    ))
                }
                Ok(n) => written += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Flush the connection
    pub fn flush(&mut self) -> io::Result<()> {
        self.connection.flush()
    }

    /// Send an error to the client
    pub fn send_error(&mut self, error: X11Error) -> io::Result<()> {
        let mut buf = [0u8; 32];
        error.encode(&mut buf);
        self.write_all(&buf)?;
        self.flush()
    }

    /// Increment the sequence number
    pub fn increment_sequence(&mut self) {
        self.sequence_number = self.sequence_number.wrapping_add(1);
    }

    /// Send raw bytes to the client
    pub fn send_raw(&mut self, data: &[u8]) -> io::Result<()> {
        self.write_all(data)?;
        self.flush()
    }
}
