//! Async pipe reading utilities for Unix.

use std::io;
use std::os::unix::io::OwnedFd;

use tokio::net::unix::pipe;

pub(crate) struct AsyncPipeReader(pipe::Receiver);

impl AsyncPipeReader {
    pub(crate) fn new(reader: std::io::PipeReader) -> io::Result<Self> {
        Ok(Self(pipe::Receiver::from_file(std::fs::File::from(
            OwnedFd::from(reader),
        ))?))
    }

    pub(crate) async fn read_to_string(&mut self) -> io::Result<String> {
        use tokio::io::AsyncReadExt;
        if crate::process_scope::active() {
            let mut bytes = Vec::new();
            (&mut self.0)
                .take(1024 * 1024)
                .read_to_end(&mut bytes)
                .await?;
            return Ok(String::from_utf8_lossy(&bytes).into_owned());
        }
        let mut s = String::new();
        self.0.read_to_string(&mut s).await?;
        Ok(s)
    }
}
