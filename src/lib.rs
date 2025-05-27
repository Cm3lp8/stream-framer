//! # Stream Framer
//!
//! Framing system for streaming protocols.
//!
//! ## Purpose
//!
//! Stream Framer is a Rust crate that provides two traits for adding header prefixes to datagrams:
//!
//! - ```FrameWriter``` prepends a header composed of 8 arbitrary bytes followed by 4 bytes representing (in big endian) the length of the following frame.
//! - ```FrameParser``` provides the method ```parse_frame_header()```to parse incoming packets, indicating the frame's starting point and its length.
//!
//! ## Disclaimers
//! - It is a very simplistic crate that currently have no mechanism to handle data coming in a corrupted order.
//! - It can handle truncated frames (e.g. a frame that is distributed between two packets).
//! I use it in the context of the QUIC protocol (with a HTTP/3 framework based on ```Quiche``` crate), which garantees data order accuracy.
//!
//! ## Example
//! Add the header (magic number: 8 bytes + frame len big endian u32: 4 bytes).
//! ```rust
//! use stream_framer::{FrameParser, ParsedStreamData};
//! use stream_framer::FrameWriter;
//!
//! // Vec<u8> implements FrameParser and FrameWriter.
//! let datagram: Vec<u8> = vec![1; 512];
//!
//! let prefixed_datagram = datagram.prepend_frame();
//! ```
//!
//! Then you can parse and handle truncations:
//! ```rust
//! use stream_framer::{FrameParser, ParsedStreamData};
//! use stream_framer::FrameWriter;
//! // states that keep track of truncated datas (for header and the frame)
//!
//! let mut incompleted_stream_data_buffer: Option<(usize, Vec<u8>)> = None; // (frame_size, partial data already received);
//! let mut truncated_header_buffer: Option<Vec<u8>> = None; // The partial header truncated in the previous packet parsing.
//!
//! let datagram: Vec<u8> = vec![1; 512];
//!
//! let prefixed_datagram = datagram.prepend_frame();
//!
//! let mut output: Vec<Vec<u8>> = vec![];
//!
//!            // Start parsing, it can output multiple frames if any.
//!            match prefixed_datagram.parse_frame_header(
//!                incompleted_stream_data_buffer.take(),
//!                truncated_header_buffer.take(),
//!            ) {
//!                Ok(parsing_res) => {
//!                    for parsed in parsing_res {
//!                        match parsed {
//!                               ParsedStreamData::Completed(data) => {
//!
//!                                   output.push(data);
//!
//!
//!                               }
//!                               ParsedStreamData::Incompleted(size, data) => {
//!
//!                                   incompleted_stream_data_buffer = Some((size, data));
//!
//!                               }
//!                               ParsedStreamData::TruncatedHeader(truncated_hdr) => {
//!
//!                                   truncated_header_buffer = Some(truncated_hdr);
//!
//!                               }
//!                      }
//!                    }
//!                }
//!                Err(e) => {
//!                    println!("[{:?}]", e);
//!                }
//!            }
//!
//!
//! ```

mod error;
mod stream_frame;
mod test;

pub use stream_frame::FrameParser;
pub use stream_frame::FrameWriter;
pub use stream_frame::ParsedStreamData;

pub mod prelude {
    pub use super::FrameParser;
    pub use super::FrameWriter;
    pub use super::ParsedStreamData;
}
