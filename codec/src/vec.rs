use crate::{BufferDecoder, BufferEncoder, Encoder};
use alloc::vec::Vec;

///
/// We encode dynamic arrays as following:
/// - header
/// - + length - number of elements inside vector
/// - + offset - offset inside structure
/// - + size - number of encoded bytes
/// - body
/// - + raw bytes of the vector
impl<T: Default + Sized + Encoder<T>> Encoder<Vec<T>> for Vec<T> {
    // u32: length + values (bytes)
    const HEADER_SIZE: usize = core::mem::size_of::<u32>() * 3;

    fn encode(&self, encoder: &mut BufferEncoder, field_offset: usize) {
        encoder.write_u32(field_offset, self.len() as u32);
        let mut value_encoder = BufferEncoder::new(T::HEADER_SIZE * self.len(), None);
        for (i, obj) in self.iter().enumerate() {
            obj.encode(&mut value_encoder, T::HEADER_SIZE * i);
        }
        encoder.write_bytes(field_offset + 4, value_encoder.finalize().as_slice());
    }

    fn decode_header(
        decoder: &mut BufferDecoder,
        field_offset: usize,
        result: &mut Vec<T>,
    ) -> (usize, usize) {
        let input_len = decoder.read_u32(field_offset) as usize;
        result.reserve(input_len);
        let (offset, length) = decoder.read_bytes_header(field_offset + 4);
        (offset, length)
    }

    fn decode_body(decoder: &mut BufferDecoder, field_offset: usize, result: &mut Vec<T>) {
        let input_len = decoder.read_u32(field_offset) as usize;
        let input_bytes = decoder.read_bytes(field_offset + 4);
        let mut value_decoder = BufferDecoder::new(input_bytes);
        *result = (0..input_len)
            .map(|i| {
                let mut result = T::default();
                T::decode_body(&mut value_decoder, T::HEADER_SIZE * i, &mut result);
                result
            })
            .collect()
    }
}
