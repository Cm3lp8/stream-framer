pub use stream_frame_parse::{FrameParser, ParsedStreamData};
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

    type BodyLen = usize;
    type ReceivedSize = usize;
    const HDR_SIZE: usize = 4; // u32
    pub trait FrameParser {
        fn parse_frame_header(
            self,
            last_incomplete_reception: Option<(BodyLen, ReceivedSize)>,
        ) -> Result<Vec<ParsedStreamData>, ()>;
    }

    pub enum ParsedStreamData {
        IncompleteWithoutHeader(Vec<u8>),
        CompletedWithHeader(usize, Vec<u8>),
        CompletedWithoutHeader(usize, Vec<u8>), // If the stream started without header first.
        IncompleteWithHeader(usize, Vec<u8>),
    }

    enum HeaderParsing {
        Completed(usize, Vec<u8>),
        OneMessageAndRemains((usize, Vec<u8>), Vec<u8>), // (size_first_msg, data_first_message),
        // remaining data
        DataTooLittle(Vec<u8>),
        StartWithNoHeaderAndRemains((usize, Vec<u8>), Vec<u8>), // (no header data bytes
        StartWithNoHeaderAndFinished(usize, Vec<u8>),           // (no header data bytes
        // size, data_without_header), Option<(hdr->data_size, next_data)>
        None,
    }

    impl FrameParser for Vec<u8> {
        // decode header, builded with u32 as bytes collection, big endian
        fn parse_frame_header(
            mut self,
            last_incomplete_reception: Option<(BodyLen, ReceivedSize)>,
        ) -> Result<Vec<ParsedStreamData>, ()> {
            let mut output: Vec<ParsedStreamData> = vec![];
            let mut data = std::mem::replace(&mut self, vec![]);

            'parse: loop {
                if data.len() < HDR_SIZE {
                    output.push(ParsedStreamData::IncompleteWithoutHeader(data));
                    return Ok(output);
                }

                let hdr: [u8; HDR_SIZE] = if let Ok(hdr) = data[0..HDR_SIZE].try_into() {
                    hdr
                } else {
                    return Err(());
                };

                if is_header(&hdr) {
                    // It starts directly with a header :
                    // case 0: total_len == body_len,
                    // case 1: total_len > body_len,
                    // case 2: total_len < body_len.

                    let body_total_len = u32::from_be_bytes(hdr);
                    let body = data[HDR_SIZE..].to_vec();

                    match has_hdr_first(body_total_len as usize, body) {
                        // final case, return.
                        HeaderParsing::Completed(_size, completed_data) => {
                            output.push(ParsedStreamData::CompletedWithHeader(
                                body_total_len as usize,
                                completed_data,
                            ));
                            return Ok(output);
                        }
                        // One first message completed, and remaining data, continue parsing
                        HeaderParsing::OneMessageAndRemains(
                            (first_msg_size, first_msg_data),
                            remaining_data,
                        ) => {
                            data = remaining_data;
                            output.push(ParsedStreamData::CompletedWithHeader(
                                first_msg_size,
                                first_msg_data,
                            ));

                            continue 'parse;
                        }
                        // Final case
                        HeaderParsing::DataTooLittle(data) => {
                            // Is inferior to the announced body len : the remaining will be in the
                            // next server push, which will have no hdr

                            output.push(ParsedStreamData::IncompleteWithHeader(
                                body_total_len as usize,
                                data,
                            ));
                            return Ok(output);
                        }
                        _ => return Err(()),
                    }
                } else {
                    // It starts with no hdr.
                    // case 3: It has 1 header pattern after some bytes.

                    match has_no_hdr_start(data, last_incomplete_reception) {
                        Ok(data_parsed) => match data_parsed {
                            HeaderParsing::StartWithNoHeaderAndRemains(
                                (size, data_wo_hdr),
                                remaining,
                            ) => {
                                data = remaining;
                                output.push(ParsedStreamData::IncompleteWithoutHeader(data_wo_hdr));
                            }
                            // Final case
                            HeaderParsing::StartWithNoHeaderAndFinished(size, data) => {
                                output.push(ParsedStreamData::IncompleteWithoutHeader(data));
                                return Ok(output);
                            }
                            _ => return Err(()),
                        },
                        Err(data_err) => data = data_err,
                    }
                };
            }
        }
    }
    fn is_header(hdr: &[u8]) -> bool {
        for i in hdr[..HDR_SIZE - 1].iter() {
            if *i != 0 {
                return false;
            }
        }
        true
    }
    fn has_no_hdr_start(
        data: Vec<u8>,
        last_incomplete_reception: Option<(BodyLen, ReceivedSize)>,
    ) -> Result<HeaderParsing, Vec<u8>> {
        match last_incomplete_reception {
            Some((msg_len, already_received)) => {
                let total_packet_len = data.len();

                let remaining_bytes = msg_len - already_received;

                // case 0 stream len contains the end of the last incomplete + at least the start
                // of the following message.
                Ok(data)
            }
            None => Err(data),
        }
    }

    fn has_hdr_first(msg_len: usize, mut data: Vec<u8>) -> HeaderParsing {
        // It starts directly with a header :
        // case 0: total_len == body_len,
        // case 1: total_len > body_len,
        // case 2: total_len < body_len.

        // case 0
        if data.len() == msg_len {
            return HeaderParsing::Completed(msg_len, data);
        }

        if data.len() > msg_len {
            let remaining = data.split_off(msg_len);
            return HeaderParsing::OneMessageAndRemains((msg_len, data), remaining);
        }

        if data.len() < msg_len {
            return HeaderParsing::DataTooLittle(data);
        }
        HeaderParsing::None
    }
}
