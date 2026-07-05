use crate::error::CompressionError;

#[inline]
pub(crate) fn encode(value: u64, buf: &mut Vec<u8>) {
    let mut val = value;
    while val >= 0x80 {
        buf.push((val as u8) | 0x80);
        val >>= 7;
    }
    buf.push(val as u8);
}

#[inline]
pub(crate) fn decode(buf: &[u8]) -> Result<(u64, usize), CompressionError> {
    let mut value = 0u64;
    let mut shift = 0;
    let mut offset = 0;

    loop {
        if offset >= buf.len() {
            return Err(CompressionError::DecompressionFailed(
                "truncated varint".to_string(),
            ));
        }

        if shift >= 64 {
            return Err(CompressionError::DecompressionFailed(
                "varint too large".to_string(),
            ));
        }

        let byte = buf[offset];
        offset += 1;
        let payload = (byte & 0x7F) as u64;
        if shift == 63 && payload > 1 {
            return Err(CompressionError::DecompressionFailed(
                "varint too large".to_string(),
            ));
        }
        value |= payload << shift;

        if (byte & 0x80) == 0 {
            break;
        }
        shift += 7;
    }

    Ok((value, offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_boundary_values() {
        for value in [0, 1, 127, 128, 16_383, 16_384, u32::MAX as u64, u64::MAX] {
            let mut encoded = Vec::new();
            encode(value, &mut encoded);
            let (decoded, consumed) = decode(&encoded).unwrap();

            assert_eq!(decoded, value);
            assert_eq!(consumed, encoded.len());
        }
    }

    #[test]
    fn rejects_truncated_varint() {
        let err = decode(&[0x80]).unwrap_err();
        assert!(matches!(err, CompressionError::DecompressionFailed(_)));
    }

    #[test]
    fn rejects_overlong_varint() {
        let err = decode(&[0x80; 11]).unwrap_err();
        assert!(matches!(err, CompressionError::DecompressionFailed(_)));
    }
}
