use super::CodecError;
use crate::ir::Segment;
use std::io::{BufReader, Read};

pub struct JsonSegmentsIterator<R: Read> {
    reader: BufReader<R>,
    started: bool,
    done: bool,
}

impl<R: Read> Iterator for JsonSegmentsIterator<R> {
    type Item = Result<Segment, CodecError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        if !self.started {
            self.started = true;
            if let Err(e) = self.skip_to_segments() {
                self.done = true;
                return Some(Err(e));
            }
        }

        match self.skip_whitespace_and_comma() {
            Ok(true) => {
                self.done = true;
                None
            }
            Ok(false) => {
                let mut de = serde_json::Deserializer::from_reader(&mut self.reader);
                match serde::Deserialize::deserialize(&mut de) {
                    Ok(seg) => Some(Ok(seg)),
                    Err(e) => {
                        self.done = true;
                        Some(Err(CodecError::Other(e.to_string())))
                    }
                }
            }
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

impl<R: Read> JsonSegmentsIterator<R> {
    pub fn new(reader: R) -> Self {
        JsonSegmentsIterator {
            reader: BufReader::new(reader),
            started: false,
            done: false,
        }
    }

    fn skip_to_segments(&mut self) -> Result<(), CodecError> {
        let pattern = b"\"segments\"";
        let mut matched = 0;
        let mut buf = [0u8; 1];
        loop {
            self.reader
                .read_exact(&mut buf)
                .map_err(|e| CodecError::Other(e.to_string()))?;
            if buf[0] == pattern[matched] {
                matched += 1;
                if matched == pattern.len() {
                    break;
                }
            } else {
                matched = 0;
                if buf[0] == pattern[0] {
                    matched = 1;
                }
            }
        }
        loop {
            self.reader
                .read_exact(&mut buf)
                .map_err(|e| CodecError::Other(e.to_string()))?;
            if buf[0] == b'[' {
                break;
            }
        }
        Ok(())
    }

    fn skip_whitespace_and_comma(&mut self) -> Result<bool, CodecError> {
        use std::io::BufRead;
        loop {
            let available = self
                .reader
                .fill_buf()
                .map_err(|e| CodecError::Other(e.to_string()))?;
            if available.is_empty() {
                return Err(CodecError::Truncated);
            }
            let c = available[0];
            if c.is_ascii_whitespace() || c == b',' {
                self.reader.consume(1);
            } else if c == b']' {
                self.reader.consume(1);
                return Ok(true);
            } else {
                return Ok(false);
            }
        }
    }
}
