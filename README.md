# Stream Framer

## Purpose

Stream Framer presents two traits that can add header prefixes to datagrams:

- ```FrameWriter``` prepends a header composed of 8 arbitrary bytes followed by 4 bytes representing in big endian the length of the following frame.
- ```FrameParser``` gives the method ```parse_frame_header()```to parse incoming packets, pointing on frame starting point and its length.

## Disclaimer
- It is a very simplistic crate that currently have no mechanism that handle data coming in corrupted order.
I use it in the context of the quic protocol (with a http3 frameword based on ```Quiche``` crate), which garanty data's order accurracy.
- It can however handle truncated frames (e.g. a frame that is distributed between two packets).
