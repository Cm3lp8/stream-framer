pub use stream_frame_parse::FrameParser;
pub use stream_frame_writer::FrameWriter;
mod stream_frame_writer {
    pub trait FrameWriter {
        fn prepend_frame_in_place(&mut self);
        fn prepend_frame(self) -> Vec<u8>;
    }

    impl FrameWriter for Vec<u8> {
        fn prepend_frame_in_place(&mut self) {
            // cast to u32 because usize is to large and to suitable (diffence of size between archs)
            let p_len = self.len() as u32;

            let mut frame: Vec<u8> = p_len.to_be_bytes().to_vec();

            frame.append(self);

            *self = frame;
        }
        fn prepend_frame(self) -> Vec<u8> {
            // cast to u32 because usize is to large and to suitable (diffence of size between archs)
            let p_len = self.len() as u32;

            let mut frame: Vec<u8> = p_len.to_be_bytes().to_vec();

            frame.extend(self);

            frame
        }
    }
}

mod stream_frame_parse {
    use crate::error::FrameError;

    pub trait FrameParser {
        fn parse_frame_header(self) -> Result<(usize, Vec<u8>), FrameError>;
    }

    impl FrameParser for Vec<u8> {
        fn parse_frame_header(self) -> Result<(usize, Vec<u8>), FrameError> {
            let hdr: [u8; 4] = if let Ok(hdr) = self[0..4].try_into() {
                hdr
            } else {
                return Err(FrameError::ParsingError(
                    "Failed to parse header".to_string(),
                ));
            };

            let body = self[4..].to_vec();

            let body_total_len = u32::from_be_bytes(hdr);

            Ok((body_total_len as usize, body))
        }
    }
}
