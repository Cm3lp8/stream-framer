pub use stream_frame_parse::{FrameParser, ParsedStreamData};
pub use stream_frame_writer::FrameWriter;
const HDR_SIZE: usize = 12; // u32
const MAGIC_PREFIX: [u8; 8] = [0x00, 0xF1, 0x01, 0xE4, 0x02, 0xFF, 0x03, 0xDD];
mod stream_frame_writer {
    use super::MAGIC_PREFIX;

    pub trait FrameWriter {
        fn prepend_frame_in_place(&mut self);
        fn prepend_frame(self) -> Vec<u8>;
    }

    impl FrameWriter for Vec<u8> {
        fn prepend_frame_in_place(&mut self) {
            // cast to u32 because usize is to large and to suitable (diffence of size between archs)
            let p_len = self.len() as u32;

            let mut frame: Vec<u8> = p_len.to_be_bytes().to_vec();

            let mut magic_prefix = MAGIC_PREFIX.to_vec();
            magic_prefix.append(&mut frame);
            magic_prefix.append(self);

            *self = magic_prefix;
        }
        fn prepend_frame(self) -> Vec<u8> {
            // cast to u32 because usize is to large and to suitable (diffence of size between archs)
            let p_len = self.len() as u32;

            let mut magic_prefix = MAGIC_PREFIX.to_vec();
            let mut frame: Vec<u8> = p_len.to_be_bytes().to_vec();
            magic_prefix.append(&mut frame);
            magic_prefix.extend(self);

            magic_prefix
        }
    }
}

mod stream_frame_parse {
    use crate::error::FrameError;

    use super::{HDR_SIZE, MAGIC_PREFIX};

    type BodyLen = usize;
    type ReceivedSize = usize;
    pub trait FrameParser {
        fn parse_frame_header(
            self,
            last_incomplete_reception: Option<(BodyLen, Vec<u8>)>,
            is_last_header_truncated: Option<Vec<u8>>,
        ) -> Result<Vec<ParsedStreamData>, ()>;
    }

    pub enum ParsedStreamData {
        IncompleteWithoutHeaderUnFinished(Vec<u8>),
        CompletedWithHeader(usize, Vec<u8>),
        IncompleteWithHeader(usize, Vec<u8>),
        TruncatedHeader(Vec<u8>),
    }

    enum HeaderParsing {
        Completed(usize, Vec<u8>),
        OneMessageAndRemains((usize, Vec<u8>), Vec<u8>), // (size_first_msg, data_first_message),
        // remaining data
        DataTooLittle(Vec<u8>),
        StartWithNoHeaderAndIncompleted(Vec<u8>), // (no header data bytes
        // size, data_without_header), Option<(hdr->data_size, next_data)>
        None,
    }

    impl FrameParser for Vec<u8> {
        // decode header, builded with u32 as bytes collection, big endian
        fn parse_frame_header(
            mut self,
            mut last_incomplete_reception: Option<(BodyLen, Vec<u8>)>,
            mut is_last_header_truncated: Option<Vec<u8>>,
        ) -> Result<Vec<ParsedStreamData>, ()> {
            let mut output: Vec<ParsedStreamData> = vec![];
            let mut data = std::mem::replace(&mut self, vec![]);

            'parse: loop {
                // when data is now < HDR_SIZE, implies truncating next header
                // what if is_last_header_truncated is_some() ? (To resolve)
                match is_data_size_truncating_next_header(data) {
                    Truncating::Yes(truncated_hdr) => {
                        output.push(ParsedStreamData::TruncatedHeader(truncated_hdr));
                        return Ok(output);
                    }

                    Truncating::No(data_continue) => {
                        data = data_continue;
                    }
                }

                // vec len is already verified to be  > HDR_SIZE
                let hdr = match compose_header_candidat(&data, is_last_header_truncated.take()) {
                    Ok(hdr) => hdr,
                    Err(_e) => return Err(()),
                };

                if is_header(&hdr) {
                    // It starts directly with a header :
                    // case 0: total_len == body_len,
                    // case 1: total_len > body_len,
                    // case 2: total_len < body_len.

                    let encoded_len: [u8; 4] = if let Ok(e_l) = hdr[MAGIC_PREFIX.len()..].try_into() {e_l}else {return Err(())};
                    let body_total_len = u32::from_be_bytes(encoded_len);
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
                            data = remaining_data; // check if remaining data is > hdr size
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

                    match has_no_hdr_start(data, last_incomplete_reception.take()) {
                        Ok(data_parsed) => match data_parsed {
                            HeaderParsing::OneMessageAndRemains((size, data_wo_hdr), remaining) => {
                                data = remaining; // TODO handle case 0.0 => See if hdr is
                                // truncated
                                output
                                    .push(ParsedStreamData::CompletedWithHeader(size, data_wo_hdr));
                            }
                            // Final case
                            HeaderParsing::Completed(size, completed_data) => {
                                output.push(ParsedStreamData::CompletedWithHeader(
                                    size as usize,
                                    completed_data,
                                ));
                                return Ok(output);
                            }
                            HeaderParsing::StartWithNoHeaderAndIncompleted(data) => {
                                // case 2 => if the msg len > stream len
                                output.push(ParsedStreamData::IncompleteWithoutHeaderUnFinished(
                                    data,
                                ));
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
    // verify if there is already a truncated header from the last packet. If yes compose a header
    // with it, if no, output header size Vec<u8>
    fn compose_header_candidat(
        data: &[u8],
        is_last_header_truncated: Option<Vec<u8>>,
    ) -> Result<[u8; HDR_SIZE], String> {
        let output_res: Result<[u8; HDR_SIZE], String> = match is_last_header_truncated {
            Some(mut hdr_prefix) => {
                let hdr_suffix_len = HDR_SIZE - hdr_prefix.len();

                let suffix_slice = data[0..hdr_suffix_len].to_vec();

                hdr_prefix.extend(suffix_slice);

                hdr_prefix
                    .try_into()
                    .map_err(|_e| "Failed to build [u8] from vec<u8>".to_string())
            }

            None => data
                .try_into()
                .map_err(|_e| "Failed to build [u8] from vec<u8>".to_string()),
        };
        output_res
    }
    fn is_header(hdr: &[u8]) -> bool {
        if hdr.len() != HDR_SIZE {
            return false;
        }

        let has_magic_prefix_slice = &hdr[..MAGIC_PREFIX.len()];

        if has_magic_prefix_slice != &MAGIC_PREFIX {
            return false;
        }
        true
    }
    type TruncatedSize = usize;
    enum Truncating {
        Yes(Vec<u8>),
        No(Vec<u8>),
    }
    fn is_data_size_truncating_next_header(data: Vec<u8>) -> Truncating {
        if data.len() < HDR_SIZE {
            Truncating::Yes(data)
        } else {
            Truncating::No(data)
        }
    }
    fn has_no_hdr_start(
        mut data: Vec<u8>,
        last_incomplete_reception: Option<(BodyLen, Vec<u8>)>,
    ) -> Result<HeaderParsing, Vec<u8>> {
        match last_incomplete_reception {
            Some((msg_len, mut already_received)) => {
                let total_packet_len = data.len();

                // the remaining bytes quantity that we should find in this packet.
                let remaining_bytes = msg_len - already_received.len();

                // case 0 stream len contains the end of the last incomplete + at least the start
                // of the following message.

                if remaining_bytes < total_packet_len {
                    let remaining = data.split_off(remaining_bytes);
                    already_received.extend(data);
                    return Ok(HeaderParsing::OneMessageAndRemains(
                        (msg_len, already_received),
                        remaining,
                    ));
                }
                if remaining_bytes == total_packet_len {
                    already_received.extend(data);
                    return Ok(HeaderParsing::Completed(msg_len, already_received));
                }
                already_received.extend(data);
                // Is extending the last packet 's data
                Ok(HeaderParsing::StartWithNoHeaderAndIncompleted(
                    already_received,
                ))
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
