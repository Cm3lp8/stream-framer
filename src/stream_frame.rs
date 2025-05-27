#![allow(clippy::single_match)]
#![allow(unused_assignments)]
#![allow(clippy::single_match_else)]
#![allow(clippy::ref_option)]

pub use stream_frame_parse::{FrameParser, ParsedStreamData};
pub use stream_frame_writer::FrameWriter;
pub const HDR_SIZE: usize = 12; // u32
pub const MAGIC_PREFIX: [u8; 8] = [0x00, 0xF1, 0x01, 0xE4, 0x02, 0xFF, 0x03, 0xDD];
mod stream_frame_writer {
    use crate::error::FrameError;

    use super::MAGIC_PREFIX;

    pub trait FrameWriter {
        /// # Errors
        /// This returns an errors if the packet length is > to u32 capacity.
        fn prepend_frame_in_place(&mut self) -> Result<(), FrameError>;
        /// # Errors
        /// This returns an errors if the packet length is > to u32 capacity.
        fn prepend_frame(self) -> Result<Vec<u8>, FrameError>;
    }

    impl FrameWriter for Vec<u8> {
        fn prepend_frame_in_place(&mut self) -> Result<(), FrameError> {
            // cast to u32 because usize is to large and to suitable (diffence of size between archs)
            let Ok(p_len) = u32::try_from(self.len()) else {
                return Err(FrameError::TypeCapacity(
                    "Failed to get packet len (is > to u32 capacity)".to_string(),
                ));
            };

            let mut frame: Vec<u8> = p_len.to_be_bytes().to_vec();

            let mut magic_prefix = MAGIC_PREFIX.to_vec();
            magic_prefix.append(&mut frame);
            magic_prefix.append(self);

            *self = magic_prefix;
            Ok(())
        }
        fn prepend_frame(self) -> Result<Vec<u8>, FrameError> {
            // cast to u32 because usize is to large and to suitable (diffence of size between archs)
            let Ok(p_len) = u32::try_from(self.len()) else {
                return Err(FrameError::TypeCapacity(
                    "Failed to get packet len (is > to u32 capacity)".to_string(),
                ));
            };

            let mut magic_prefix = MAGIC_PREFIX.to_vec();
            let mut frame: Vec<u8> = p_len.to_be_bytes().to_vec();
            magic_prefix.append(&mut frame);
            magic_prefix.extend(self);

            Ok(magic_prefix)
        }
    }
}

mod stream_frame_parse {

    use crate::error::FrameError;

    use super::{HDR_SIZE, MAGIC_PREFIX};

    type BodyLen = usize;
    pub trait FrameParser {
        /// Parse a stream packet
        /// # Errors
        /// Return a String in case something wrong happened in
        /// slice conversions.
        ///
        fn parse_frame_header(
            self,
            last_incomplete_reception: Option<(BodyLen, Vec<u8>)>,
            is_last_header_truncated: Option<Vec<u8>>,
        ) -> Result<Vec<ParsedStreamData>, FrameError>;
    }

    type MessageSize = usize;
    pub enum ParsedStreamData {
        Completed(Vec<u8>),
        Incompleted(MessageSize, Vec<u8>),
        TruncatedHeader(Vec<u8>), // bool +> end of stream
    }

    enum HeaderParsing {
        Completed(usize, Vec<u8>),
        OneMessageAndRemains((usize, Vec<u8>), Vec<u8>), // (size_first_msg, data_first_message),
        // remaining data
        DataTooLittle(Vec<u8>),
        StartWithNoHeaderAndIncompleted(MessageSize, Vec<u8>), // (no header data bytes
        // size, data_without_header), Option<(hdr->data_size, next_data)>
        None,
    }

    impl FrameParser for Vec<u8> {
        // decode header, builded with u32 as bytes collection, big endian
        fn parse_frame_header(
            mut self,
            mut last_incomplete_reception: Option<(BodyLen, Vec<u8>)>,
            mut is_last_header_truncated: Option<Vec<u8>>,
        ) -> Result<Vec<ParsedStreamData>, FrameError> {
            let mut output: Vec<ParsedStreamData> = vec![];
            let mut data = std::mem::take(&mut self);
            let data_len = data.len();

            'parse: loop {
                if data_len == 0 {
                    break 'parse Ok(output);
                }
                let mut start_of_stream = false;
                // when data is now < HDR_SIZE, implies truncating next header
                // what if is_last_header_truncated is_some() ? (To resolve)
                match is_data_size_truncating_next_header(
                    data,
                    &last_incomplete_reception,
                    &mut is_last_header_truncated,
                ) {
                    Truncating::Yes(truncated_hdr) => {
                        if !truncated_hdr.is_empty() {
                            output.push(ParsedStreamData::TruncatedHeader(truncated_hdr));
                        }
                        return Ok(output);
                    }

                    Truncating::No((data_continue, s_o_s)) => {
                        data = data_continue;
                        start_of_stream = s_o_s;
                    }
                }

                // vec len is already verified to be  > HDR_SIZE

                if start_of_stream {
                    let (corriged_hdr_len, hdr) =
                        compose_header_candidat(&data, is_last_header_truncated.take())?;

                    if is_header(&hdr) {
                        // It starts directly with a header :
                        // case 0: total_len == body_len,
                        // case 1: total_len > body_len,
                        // case 2: total_len < body_len.

                        let encoded_len: [u8; 4] =
                            if let Ok(e_l) = hdr[MAGIC_PREFIX.len()..].try_into() {
                                e_l
                            } else {
                                return Err(FrameError::ParsingError(
                                    "error decoding header".to_string(),
                                ));
                            };
                        let body_total_len = u32::from_be_bytes(encoded_len);

                        let body = data[corriged_hdr_len..].to_vec();

                        match has_hdr_first(body_total_len as usize, body) {
                            // final case, return.
                            HeaderParsing::Completed(_size, completed_data) => {
                                output.push(ParsedStreamData::Completed(completed_data));
                                return Ok(output);
                            }
                            // One first message completed, and remaining data, continue parsing
                            HeaderParsing::OneMessageAndRemains(
                                (_first_msg_size, first_msg_data),
                                remaining_data,
                            ) => {
                                data = remaining_data; // check if remaining data is > hdr size
                                output.push(ParsedStreamData::Completed(first_msg_data));

                                continue 'parse;
                            }
                            // Final case
                            HeaderParsing::DataTooLittle(data) => {
                                // Is inferior to the announced body len : the remaining will be in the
                                // next server push, which will have no hdr

                                output.push(ParsedStreamData::Incompleted(
                                    body_total_len as usize,
                                    data,
                                ));
                                return Ok(output);
                            }
                            _ => return Err(FrameError::ParsingError("Other case".to_string())),
                        }
                    }
                    // It starts with no hdr.
                    // case 3: It has 1 header pattern after some bytes.
                }
                match has_no_hdr_start(data, last_incomplete_reception.take()) {
                    Ok(data_parsed) => match data_parsed {
                        HeaderParsing::OneMessageAndRemains((_size, data_wo_hdr), remaining) => {
                            data = remaining; // TODO handle case 0.0 => See if hdr is
                            // truncated
                            output.push(ParsedStreamData::Completed(data_wo_hdr));
                        }
                        // Final case
                        HeaderParsing::Completed(_size, completed_data) => {
                            output.push(ParsedStreamData::Completed(completed_data));
                            return Ok(output);
                        }
                        HeaderParsing::StartWithNoHeaderAndIncompleted(message_size, data) => {
                            // case 2 => if the msg len > stream len
                            output.push(ParsedStreamData::Incompleted(message_size, data));
                            return Ok(output);
                        }
                        _ => {
                            return Err(FrameError::ParsingError(
                                "other case after no header".to_string(),
                            ));
                        }
                    },
                    Err(data_err) => data = data_err,
                }
            }
        }
    }

    type SuffixLen = usize;
    // verify if there is already a truncated header from the last packet. If yes compose a header
    // with it, if no, output header size Vec<u8>
    fn compose_header_candidat(
        data: &[u8],
        is_last_header_truncated: Option<Vec<u8>>,
    ) -> Result<(SuffixLen, [u8; HDR_SIZE]), FrameError> {
        let output_res: Result<(SuffixLen, [u8; HDR_SIZE]), FrameError> =
            match is_last_header_truncated {
                Some(mut hdr_prefix) => {
                    let mut hdr_suffix_len = 0;
                    if hdr_prefix.len() >= HDR_SIZE {
                        hdr_suffix_len = HDR_SIZE;
                    } else {
                        hdr_suffix_len = HDR_SIZE - hdr_prefix.len();

                        if hdr_suffix_len != 0 {
                            let suffix_slice = data[0..hdr_suffix_len].to_vec();
                            hdr_prefix.extend(suffix_slice);
                        } else {
                            hdr_suffix_len = HDR_SIZE;
                        }
                    }

                    let hdr: [u8; HDR_SIZE] = hdr_prefix[..HDR_SIZE].try_into().map_err(|_e| {
                        FrameError::TypeConversionFailure(
                            "Failed to build [u8] from vec<u8>".to_string(),
                        )
                    })?;

                    Ok((hdr_suffix_len, hdr))
                }

                None => {
                    let hdr: [u8; HDR_SIZE] = data[..HDR_SIZE]
                        .try_into()
                        .map_err(|e| FrameError::TypeConversionFailure(format!("[{e:?}]")))?;

                    Ok((HDR_SIZE, hdr))
                }
            };
        output_res
    }
    fn is_header(hdr: &[u8]) -> bool {
        if hdr.len() != HDR_SIZE {
            return false;
        }

        let has_magic_prefix_slice = &hdr[..MAGIC_PREFIX.len()];

        if has_magic_prefix_slice != MAGIC_PREFIX {
            return false;
        }
        true
    }
    enum Truncating {
        Yes(Vec<u8>),
        No((Vec<u8>, bool)), // end of stream
    }
    fn is_data_size_truncating_next_header(
        data: Vec<u8>,
        last_incomplete_reception: &Option<(BodyLen, Vec<u8>)>,
        is_last_header_truncated: &mut Option<Vec<u8>>,
    ) -> Truncating {
        match last_incomplete_reception {
            Some((_size, data_received)) => {
                //  if size - data_received.len() > data.len() {
                let is_start_of_stream =
                    data_received.is_empty() && is_last_header_truncated.is_some();
                return Truncating::No((data, is_start_of_stream));
                // }
            }
            None => {}
        }
        if data.len() < HDR_SIZE {
            match is_last_header_truncated {
                Some(previous_truncation) => {
                    if previous_truncation.len() >= HDR_SIZE {
                        return Truncating::No((data, true));
                    }
                    let mut previous_extended = previous_truncation.clone();
                    let previous_len = previous_extended.len();
                    let len_max = HDR_SIZE - previous_len;

                    if data.len() < len_max {
                        previous_extended.extend_from_slice(&data);
                    } else {
                        previous_extended.extend_from_slice(&data[..len_max]);
                    }

                    *previous_truncation = previous_extended;
                    if previous_truncation.len() == HDR_SIZE {
                        previous_truncation.extend_from_slice(&data[len_max..]);
                        Truncating::No((previous_truncation.clone(), true))
                    } else {
                        Truncating::Yes(previous_truncation.clone())
                    }
                }
                None => Truncating::Yes(data),
            }
        } else {
            Truncating::No((data, true))
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
                    msg_len,
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
