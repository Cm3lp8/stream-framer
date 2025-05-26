# Stream Framer

## Purpose

Stream Framer is a Rust crate that provides two traits for adding header prefixes to datagrams:

- ```FrameWriter``` prepends a header composed of 8 arbitrary bytes followed by 4 bytes representing (in big endian) the length of the following frame.
- ```FrameParser``` provides the method ```parse_frame_header()```to parse incoming packets, indicating the frame's starting point and its length.

## Disclaimers
- It is a very simplistic crate that currently have no mechanism to handle data coming in a corrupted order.
I use it in the context of the QUIC protocol (with a http3 frameword based on ```Quiche``` crate), which garantees data order accurracy.
- It can handle truncated frames (e.g. a frame that is distributed between two packets).
