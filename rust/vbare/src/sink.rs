//! Output buffers for encoding.

use bytes::{BufMut, Bytes, BytesMut};

/// A buffer that encoded messages are written to.
///
/// `put_data` receives `data` and `str` payloads as shared [`Bytes`], so a sink that keeps
/// buffers by reference can avoid copying them. The default copies.
pub trait Sink: BufMut {
    fn put_data(&mut self, data: &Bytes) {
        self.put_slice(data);
    }
}

impl Sink for Vec<u8> {}

impl Sink for BytesMut {}

/// Writing past the end of a slice panics, as for any [`BufMut`]. Use
/// [`Encode::encode_to_slice`](crate::Encode::encode_to_slice), which checks the length first.
impl Sink for &mut [u8] {}
