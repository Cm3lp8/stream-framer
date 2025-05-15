pub use stream_frame_writer::FrameWriter;
mod stream_frame_writer {
    pub trait FrameWriter {
        fn prepend_frame_in_place(&mut self);
        fn prepend_frame(self) -> Vec<u8>;
    }

    impl FrameWriter for Vec<u8> {
        fn prepend_frame_in_place(&mut self) {
            let p_len = self.len();

            let mut frame: Vec<u8> = p_len.to_be_bytes().to_vec();

            frame.append(self);

            *self = frame;
        }
        fn prepend_frame(self) -> Vec<u8> {
            let p_len = self.len();

            let mut frame: Vec<u8> = p_len.to_be_bytes().to_vec();

            frame.extend(self);

            frame
        }
    }
}
