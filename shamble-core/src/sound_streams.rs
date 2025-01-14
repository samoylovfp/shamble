use cpal::{Device, Stream};

#[derive(Default)]
pub struct SoundState {
    _pb_dev: Option<Device>,
    _rec_dev: Option<Device>,
    _pb_stream: Option<Stream>,
    _rec_stream: Option<Stream>,
}

pub struct SoundBuffer {
    data: [f32; 512],
    length: usize,
}

pub struct EncodedBuffer {
    data: [u8; 1024],
    length: usize,
}

impl EncodedBuffer {
    pub fn slice(&self) -> &[u8] {
        &self.data[..self.length]
    }
}

impl SoundBuffer {
    pub fn slice(&self) -> &[f32] {
        &self.data[..self.length]
    }
}
