//! The two socket transports of the protocol behind one listener and one stream type, and the
//! framing: one JSON value per line.

use std::io::{self, BufRead, Read, Write};
#[cfg(not(unix))]
use std::net::TcpListener;
use std::net::{Shutdown, TcpStream, ToSocketAddrs as _};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Duration;

use serde_json::Value;

use super::discovery::Transport;
use super::pyvalue::dumps;

/// A listening socket.
pub(crate) enum Listener {
    /// A Unix domain socket (macOS, Linux).
    #[cfg(unix)]
    Unix(UnixListener),
    /// TCP on `127.0.0.1` (Windows).
    #[cfg(not(unix))]
    Tcp(TcpListener),
}

impl Listener {
    /// Waits for the next connection.
    pub(crate) fn accept(&self) -> io::Result<Stream> {
        match self {
            #[cfg(unix)]
            Self::Unix(listener) => listener.accept().map(|(stream, _)| Stream::Unix(stream)),
            #[cfg(not(unix))]
            Self::Tcp(listener) => listener.accept().map(|(stream, _)| Stream::Tcp(stream)),
        }
    }
}

/// A connected socket.
#[derive(Debug)]
pub(crate) enum Stream {
    /// A Unix domain socket (macOS, Linux).
    #[cfg(unix)]
    Unix(UnixStream),
    /// TCP: the transport on Windows, and a TCP session's address elsewhere.
    Tcp(TcpStream),
}

impl Stream {
    /// Connects to a session's address: a socket path, or `host:port` for TCP. `timeout` bounds
    /// a TCP connect; a Unix domain socket connects or fails at once.
    pub(crate) fn connect(
        transport: Transport,
        address: &str,
        timeout: Duration,
    ) -> io::Result<Self> {
        match transport {
            #[cfg(unix)]
            Transport::Unix => UnixStream::connect(address).map(Self::Unix),
            #[cfg(not(unix))]
            Transport::Unix => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Unix domain sockets are not available on this platform",
            )),
            Transport::Tcp => {
                // `address.rsplit(":", 1)` and `int(port)` in the prototype.
                let (host, port) = address
                    .rsplit_once(':')
                    .and_then(|(host, port)| Some((host, port.trim().parse::<u16>().ok()?)))
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("not a TCP address: {address}"),
                        )
                    })?;
                let target = (host, port)
                    .to_socket_addrs()?
                    .find(std::net::SocketAddr::is_ipv4)
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::NotFound,
                            format!("no IPv4 address for {host}"),
                        )
                    })?;
                TcpStream::connect_timeout(&target, timeout).map(Self::Tcp)
            }
        }
    }

    /// A second handle to the same socket (one reads, the other writes).
    pub(crate) fn try_clone(&self) -> io::Result<Self> {
        match self {
            #[cfg(unix)]
            Self::Unix(stream) => stream.try_clone().map(Self::Unix),
            Self::Tcp(stream) => stream.try_clone().map(Self::Tcp),
        }
    }

    /// Bounds every following read and write (`socket.settimeout`); `None` blocks.
    pub(crate) fn set_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Unix(stream) => {
                stream.set_read_timeout(timeout)?;
                stream.set_write_timeout(timeout)
            }
            Self::Tcp(stream) => {
                stream.set_read_timeout(timeout)?;
                stream.set_write_timeout(timeout)
            }
        }
    }

    /// Shuts down one or both directions.
    pub(crate) fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Unix(stream) => stream.shutdown(how),
            Self::Tcp(stream) => stream.shutdown(how),
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            #[cfg(unix)]
            Self::Unix(stream) => stream.read(buf),
            Self::Tcp(stream) => stream.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            #[cfg(unix)]
            Self::Unix(stream) => stream.write(buf),
            Self::Tcp(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Unix(stream) => stream.flush(),
            Self::Tcp(stream) => stream.flush(),
        }
    }
}

/// Python's `readline(limit)`: the bytes up to and including the next `\n`, at most `limit`
/// bytes, fewer at the end of the stream; empty at the end of the stream.
pub(crate) fn read_line(reader: &mut impl BufRead, limit: usize) -> io::Result<Vec<u8>> {
    let mut line = Vec::new();
    let limit = u64::try_from(limit).unwrap_or(u64::MAX);
    Read::take(reader, limit).read_until(b'\n', &mut line)?;
    Ok(line)
}

/// Writes `message` as one line, as the prototype does: `json.dumps(message) + "\n"`.
pub(crate) fn write_message(writer: &mut impl Write, message: &Value) -> io::Result<()> {
    let mut line = dumps(message);
    line.push(b'\n');
    writer.write_all(&line)?;
    writer.flush()
}

/// Reads a received line as `json.loads(bytes)` does for UTF-8 input, which skips a UTF-8 byte
/// order mark.
pub(crate) fn parse_line(line: &[u8]) -> serde_json::Result<Value> {
    serde_json::from_slice(line.strip_prefix(b"\xef\xbb\xbf").unwrap_or(line))
}
