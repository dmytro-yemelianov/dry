use super::CodecError;
use std::io::Read;

pub(super) struct Reader<'a> {
    buf: &'a [u8],
    pub(super) at: usize,
}

impl<'a> Reader<'a> {
    pub(super) fn new(buf: &'a [u8]) -> Reader<'a> {
        Reader { buf, at: 0 }
    }
    pub(super) fn take(&mut self, n: usize) -> Result<&'a [u8], CodecError> {
        let end = self.at.checked_add(n).ok_or(CodecError::Truncated)?;
        let slice = self.buf.get(self.at..end).ok_or(CodecError::Truncated)?;
        self.at = end;
        Ok(slice)
    }
    pub(super) fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.take(1)?[0])
    }
    pub(super) fn u32(&mut self) -> Result<u32, CodecError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub(super) fn f64(&mut self) -> Result<f64, CodecError> {
        let b = self.take(8)?;
        Ok(f64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }
    pub(super) fn bits(&mut self, n: usize) -> Result<Vec<bool>, CodecError> {
        let bytes = self.take(n.div_ceil(8))?;
        Ok((0..n).map(|i| (bytes[i / 8] >> (i % 8)) & 1 == 1).collect())
    }
    pub(super) fn opt_col(&mut self, n: usize) -> Result<Vec<Option<f64>>, CodecError> {
        let valid = self.bits(n)?;
        let mut col = Vec::with_capacity(n);
        for v in valid {
            let x = self.f64()?;
            col.push(if v { Some(x) } else { None });
        }
        Ok(col)
    }
    pub(super) fn col(&mut self, n: usize) -> Result<Vec<f64>, CodecError> {
        (0..n).map(|_| self.f64()).collect()
    }
    pub(super) fn opt_u32_col(&mut self, n: usize) -> Result<Vec<Option<u32>>, CodecError> {
        let valid = self.bits(n)?;
        let mut col = Vec::with_capacity(n);
        for v in valid {
            let x = self.u32()?;
            col.push(if v { Some(x) } else { None });
        }
        Ok(col)
    }
    pub(super) fn opt_vec3_col(&mut self, n: usize) -> Result<Vec<Option<[f64; 3]>>, CodecError> {
        let valid = self.bits(n)?;
        let mut col = Vec::with_capacity(n);
        for v in valid {
            let triple = [self.f64()?, self.f64()?, self.f64()?];
            col.push(if v { Some(triple) } else { None });
        }
        Ok(col)
    }
}

pub(super) fn read_error(error: std::io::Error) -> CodecError {
    if error.kind() == std::io::ErrorKind::UnexpectedEof {
        CodecError::Truncated
    } else {
        CodecError::Other(error.to_string())
    }
}

pub(super) fn read_array<const N: usize, R: Read>(reader: &mut R) -> Result<[u8; N], CodecError> {
    let mut bytes = [0u8; N];
    reader.read_exact(&mut bytes).map_err(read_error)?;
    Ok(bytes)
}

pub(super) fn read_u8<R: Read>(reader: &mut R) -> Result<u8, CodecError> {
    Ok(read_array::<1, _>(reader)?[0])
}

pub(super) fn read_u32<R: Read>(reader: &mut R) -> Result<u32, CodecError> {
    Ok(u32::from_le_bytes(read_array::<4, _>(reader)?))
}

pub(super) fn read_vec<R: Read>(reader: &mut R, len: usize) -> Result<Vec<u8>, CodecError> {
    let mut bytes = vec![0u8; len];
    reader.read_exact(&mut bytes).map_err(read_error)?;
    Ok(bytes)
}
