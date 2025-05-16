#[cfg(test)]
mod test {

    use crate::{FrameParser, FrameWriter};

    #[test]
    fn write_header() {
        let input = "Une phrase test pour voir.".to_string();
        let test_body = input.clone().as_bytes().to_vec();

        let test_body_len = test_body.len();

        let mut frame = test_body.prepend_frame();

        let frame_len = frame.len();

        assert!(frame_len != test_body_len);
        assert!(frame_len - 4 == test_body_len);

        let hdr: [u8; 4] = frame[0..4].try_into().expect("wrong len (4 bytes)");

        let body = String::from_utf8_lossy(&frame[4..]);

        let retrieved_len = u32::from_be_bytes(hdr);

        assert!(retrieved_len == test_body_len as u32);
        assert!(input == body);

        let test_body = input.clone().as_bytes().to_vec();

        let test_body_len = test_body.len();

        let mut frame = test_body.prepend_frame();

        let frame_len = frame.len();

        let res = frame.parse_frame_header();

        assert!(res.is_ok());

        let (byte_len, body) = res.expect("should have unwrap");

        assert!(byte_len == test_body_len);
        assert!(body.len() == byte_len);
        assert!(String::from_utf8_lossy(&body) == input);
    }
}
