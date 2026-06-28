use super::CodecError;
use crate::ir::{Segment, Toolpath};
use std::io::{BufReader, Read};

pub struct JsonSegmentsIterator<R: Read> {
    reader: Option<BufReader<R>>,
    segments: Option<std::vec::IntoIter<Segment>>,
    done: bool,
}

impl<R: Read> Iterator for JsonSegmentsIterator<R> {
    type Item = Result<Segment, CodecError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        if self.segments.is_none() {
            if let Err(e) = self.load_segments() {
                self.done = true;
                return Some(Err(e));
            }
        }

        match self.segments.as_mut().and_then(Iterator::next) {
            Some(segment) => Some(Ok(segment)),
            None => {
                self.done = true;
                None
            }
        }
    }
}

impl<R: Read> JsonSegmentsIterator<R> {
    pub fn new(reader: R) -> Self {
        JsonSegmentsIterator {
            reader: Some(BufReader::new(reader)),
            segments: None,
            done: false,
        }
    }

    fn load_segments(&mut self) -> Result<(), CodecError> {
        let reader = self
            .reader
            .take()
            .ok_or_else(|| CodecError::Other("JSON reader was already consumed".to_string()))?;
        let value: serde_json::Value =
            serde_json::from_reader(reader).map_err(|e| CodecError::Other(e.to_string()))?;
        let ir = value.get("ir").cloned().unwrap_or(value);
        let tp: Toolpath<Vec<Segment>> =
            serde_json::from_value(ir).map_err(|e| CodecError::Other(e.to_string()))?;
        self.segments = Some(tp.segments.into_iter());
        Ok(())
    }
}
