//! Connection layer
//!
//! This module handles network connections from X11 clients via TCP and Unix sockets.


// Allow dead code for now - skeleton implementation not yet integrated
#![allow(dead_code)]

use std::io;
use std::net::{TcpListener, TcpStream};

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

/// Connection type
pub enum Connection {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(UnixStream),
}

impl Connection {
    /// Read data from connection
    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Connection::Tcp(stream) => {
                use std::io::Read;
                stream.read(buf)
            }
            #[cfg(unix)]
            Connection::Unix(stream) => {
                use std::io::Read;
                stream.read(buf)
            }
        }
    }

    /// Write data to connection
    pub fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Connection::Tcp(stream) => {
                use std::io::Write;
                stream.write(buf)
            }
            #[cfg(unix)]
            Connection::Unix(stream) => {
                use std::io::Write;
                stream.write(buf)
            }
        }
    }

    /// Flush the connection
    pub fn flush(&mut self) -> io::Result<()> {
        match self {
            Connection::Tcp(stream) => {
                use std::io::Write;
                stream.flush()
            }
            #[cfg(unix)]
            Connection::Unix(stream) => {
                use std::io::Write;
                stream.flush()
            }
        }
    }
}

/// Connection listener
pub enum Listener {
    Tcp(TcpListener),
    #[cfg(unix)]
    Unix(UnixListener),
}

impl Listener {
    /// Create a TCP listener
    pub fn tcp(port: u16) -> io::Result<Self> {
        let addr = format!("0.0.0.0:{}", port);
        let listener = TcpListener::bind(addr)?;
        Ok(Listener::Tcp(listener))
    }

    /// Create a Unix socket listener
    #[cfg(unix)]
    pub fn unix(path: &str) -> io::Result<Self> {
        // Remove existing socket file if it exists
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path)?;
        Ok(Listener::Unix(listener))
    }

    /// Accept a new connection
    pub fn accept(&self) -> io::Result<Connection> {
        match self {
            Listener::Tcp(listener) => {
                let (stream, _) = listener.accept()?;
                Ok(Connection::Tcp(stream))
            }
            #[cfg(unix)]
            Listener::Unix(listener) => {
                let (stream, _) = listener.accept()?;
                Ok(Connection::Unix(stream))
            }
        }
    }
}
