# Stream Framer

Framing system for streaming protocols written in rust.

## Purpose

Stream Framer is a Rust crate that provides two traits for adding header prefixes to datagrams:

- ```FrameWriter``` prepends a header composed of 8 arbitrary bytes followed by 4 bytes representing (in big endian) the length of the following frame.
- ```FrameParser``` provides the method ```parse_frame_header()```to parse incoming packets, indicating the frame's starting point and its length.

## Disclaimers
- It is a very simplistic crate that currently have no mechanism to handle data coming in a corrupted order.
I use it in the context of the QUIC protocol (with a http3 frameword based on ```Quiche``` crate), which garantees data order accurracy.
- It can handle truncated frames (e.g. a frame that is distributed between two packets).

## Example 
Add the header (magic number: [u8; 8] + frame len big endian u32: [u8;4]).
```rust



 use stream_frame_parse::{FrameParser, ParsedStreamData};
 use stream_frame_writer::FrameWriter;

 // Vec<u8> implements FrameParser and FrameWriter.
 let datagram: Vec<u8> = vec![1; 512];

 let prefixed_datagram = datagram.prepend_frame();
 ```
Then you can parse and handles truncations:
```rust



 // states that keep track of truncated datas (for header and the frame)

 let mut incompleted_stream_data_buffer: Option<(usize, Vec<u8>)> = None; // (frame_size, partial data already received);
 let mut truncated_header_buffer: Option<Vec<u8>> = None; // The partial header truncated in the previous packet parsing.




 let mut output: Vec<Vec<u8>> = vec![];

            // Start parsing, it can output mutiple frames if any.
            match prefixed_datagram.parse_frame_header(
                incompleted_stream_data_buffer.take(),
                truncated_header_buffer.take(),
            ) {
                Ok(parsing_res) => {
                    for parsed in parsing_res {
                        match parsed {
                            ParsedStreamData::CompletedWithHeader(msg_size, data) => {
                                output.push(data);
                            }
                            ParsedStreamData::IncompleteWithHeader(msg_size, data) => {
                                // this has to be the last
                                // reserve for the next packet
                                self.incompleted_stream_data_buffer = Some((msg_size, data));
                            }
                            ParsedStreamData::IncompleteWithoutHeaderUnFinished(data) => {
                                // case when this packet is smaller than the message size
                            }
                            ParsedStreamData::TruncatedHeader(truncated_header) => {
                                // truncated header
                                match &self.truncated_header_buffer {
                                    Some(_partial_hdr) => {}
                                    None => self.truncated_header_buffer = Some(truncated_header),
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("[{:?}]", e);
                }
            }


 ```
