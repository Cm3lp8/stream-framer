#[cfg(test)]
mod test {

    use core::panic;
    use std::{
        sync::{Arc, atomic::AtomicUsize},
        time::Duration,
    };

    use crate::{
        FrameParser, FrameWriter, ParsedStreamData,
        stream_frame::{HDR_SIZE, MAGIC_PREFIX},
    };
    #[test]
    fn write_header() {
        // small data
        // message is in the packet

        let input = "Une phrase test pour voir.".to_string();
        let test_body = input.clone().as_bytes().to_vec();

        let test_body_len = test_body.len();

        let Ok(mut frame) = test_body.prepend_frame() else {
            panic!("failed to prepend frame")
        };

        let frame_len = frame.len();

        assert!(frame_len != test_body_len);
        assert!(frame_len - HDR_SIZE == test_body_len);

        let hdr: [u8; HDR_SIZE] = frame[0..HDR_SIZE].try_into().expect("wrong header len");

        let body = String::from_utf8_lossy(&frame[HDR_SIZE..]);

        let retrieved_len = u32::from_be_bytes(hdr[MAGIC_PREFIX.len()..].try_into().unwrap());

        assert!(retrieved_len == test_body_len as u32);
        assert!(input == body);

        let test_body = input.clone().as_bytes().to_vec();

        let test_body_len = test_body.len();

        let mut frame = test_body.prepend_frame().unwrap();

        let frame_len = frame.len();

        let mut truncated_header: Option<Vec<u8>> = None;
        let mut previous_incompleted_data: Option<(usize, Vec<u8>)> = None;

        let res =
            frame.parse_frame_header(previous_incompleted_data.take(), truncated_header.take());

        assert!(res.is_ok());

        for parsed in res.unwrap() {
            match parsed {
                ParsedStreamData::Completed(data) => {
                    assert!(data.len() == test_body_len);
                    assert!(String::from_utf8_lossy(&data) == input);
                }
                ParsedStreamData::Incompleted(size, data) => {}
                ParsedStreamData::TruncatedHeader(truncadeted_hdr) => {}
            }
        }
    }
    #[test]
    fn write_header_large_splitted_data() {
        let packet_size = 256;

        let messages_quantity = 1700000;
        let mut messages: Vec<Vec<u8>> = vec![];

        let test_content = "Une nouvelle phrase test d'un certain nombre de bytes".to_string();

        assert!(test_content.len() * messages_quantity > packet_size);

        for _i in 0..messages_quantity {
            let frame = test_content.as_bytes().to_vec().prepend_frame().unwrap();

            messages.push(frame);
        }

        // messages contenation to get one vec!

        let concatened = messages.concat();

        assert!(concatened.len() == (test_content.as_bytes().len() + HDR_SIZE) * messages_quantity);

        // split the concatenated vec in chunks of packet's size , to simulate packet streaming

        let mut stream_chunks: Vec<Vec<u8>> = vec![];

        for chunk in concatened.chunks(packet_size) {
            stream_chunks.push(chunk.to_vec());
        }

        assert!(
            stream_chunks.len() == (concatened.len() as f32 / packet_size as f32).ceil() as usize
        );

        let stream_channel = crossbeam::channel::unbounded::<Vec<u8>>();

        let sender = stream_channel.0.clone();
        let receiver = stream_channel.1.clone();

        // create a stream sender
        //
        let len = stream_chunks.len();

        std::thread::spawn(move || {
            for packet in stream_chunks {
                let _ = sender.send(packet);
            }
        });

        // client simulation :

        let thread_handle = std::thread::spawn(move || {
            let mut previous_incompleted_data: Option<(usize, Vec<u8>)> = None;
            let mut truncated_header: Option<Vec<u8>> = None;

            let mut messages_received: Vec<String> = vec![];
            let mut loop_count = 0;
            while let Ok(packet) = receiver.recv() {
                loop_count += 1;
                let parsed = match packet
                    .parse_frame_header(previous_incompleted_data.take(), truncated_header.take())
                {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        println!("er [{e:?}]");
                        vec![]
                    }
                };

                for p in parsed {
                    match p {
                        ParsedStreamData::Completed(data) => {
                            messages_received.push(String::from_utf8_lossy(&data).to_string());
                        }
                        ParsedStreamData::Incompleted(message_size, data) => {
                            previous_incompleted_data = Some((message_size, data));
                        }
                        ParsedStreamData::TruncatedHeader(truncadted_hdr) => {
                            truncated_header = Some(truncadted_hdr);
                        }
                    }
                }
                if loop_count == len {
                    break;
                }
            }

            println!(
                "number of messages [{:?}/[{:?}]]",
                messages_received.len(),
                messages_quantity
            );
            assert!(messages_received.len() == messages_quantity);
        });

        assert!(thread_handle.join().is_ok());
    }
    #[test]
    fn send_messages_larger_than_packet_size() {
        let packet_size = 128;

        let messages_quantity = 100000;
        let mut messages: Vec<Vec<u8>> = vec![];

        let test_content = "Une nouvelle phrase test d'un certain nombre de bytes, je ne sais  Lorem ipsum dolor sit amet, officia excepteur ex fugiat reprehenderit enim labore culpa sint ad nisi Lorem pariatur mollit ex esse exercitation amet. Nisi anim cupidatat excepteur officia. Reprehenderit nostrud nostrud ipsum Lorem est aliquip amet voluptate voluptate dolor minim nulla est proident. Nostrud officia pariatur ut officia. Sit irure elit esse ea nulla sunt ex occaecat reprehenderit commodo officia dolor Lorem duis laboris cupidatat officia voluptate. Culpa proident adipisicing id nulla nisi laboris ex in Lorem sunt duis officia eiusmod. Aliqua reprehenderit commodo ex non excepteur duis sunt velit enim. Voluptate laboris sint cupidatat ullamco ut ea consectetur et est culpa et culpa duis.".to_string();

        assert!(test_content.len() * messages_quantity > packet_size);
        assert!(test_content.len() > packet_size);

        for i in 0..messages_quantity {
            let frame = test_content.as_bytes().to_vec().prepend_frame().unwrap();

            messages.push(frame);
        }

        // messages contenation to get one vec!

        let concatened = messages.concat();

        assert!(concatened.len() == (test_content.as_bytes().len() + HDR_SIZE) * messages_quantity);

        // split the concatenated vec in chunks of packet's size , to simulate packet streaming

        let mut stream_chunks: Vec<Vec<u8>> = vec![];

        for chunk in concatened.chunks(packet_size) {
            stream_chunks.push(chunk.to_vec());
        }

        assert!(
            stream_chunks.len() == (concatened.len() as f32 / packet_size as f32).ceil() as usize
        );

        let stream_channel = crossbeam::channel::unbounded::<Vec<u8>>();

        let sender = stream_channel.0.clone();
        let receiver = stream_channel.1.clone();

        // create a stream sender
        //
        let len = stream_chunks.len();

        std::thread::spawn(move || {
            for packet in stream_chunks {
                let _ = sender.send(packet);
            }
        });

        // client simulation :

        let thread_handle = std::thread::spawn(move || {
            let mut previous_incompleted_data: Option<(usize, Vec<u8>)> = None;
            let mut truncated_header: Option<Vec<u8>> = None;

            let mut messages_received: Vec<String> = vec![];
            let mut loop_count = 0;
            while let Ok(packet) = receiver.recv() {
                loop_count += 1;
                let parsed = match packet
                    .parse_frame_header(previous_incompleted_data.take(), truncated_header.take())
                {
                    Ok(parsed) => parsed,

                    Err(e) => {
                        println!("er [{e:?}]");
                        vec![]
                    }
                };

                for p in parsed {
                    match p {
                        ParsedStreamData::Completed(data) => {
                            messages_received.push(String::from_utf8_lossy(&data).to_string());
                        }
                        ParsedStreamData::Incompleted(message_size, data) => {
                            previous_incompleted_data = Some((message_size, data));
                        }
                        ParsedStreamData::TruncatedHeader(truncadted_hdr) => {
                            truncated_header = Some(truncadted_hdr);
                        }
                    }
                }
                if loop_count == len {
                    break;
                }
            }

            assert!(messages_received.len() == messages_quantity);
        });

        assert!(thread_handle.join().is_ok());
    }
    #[test]
    fn send_messages_larger_than_packet_size_with_very_tiny_packet() {
        let packet_size = HDR_SIZE; //

        let messages_quantity = 1_010;
        let mut messages: Vec<Vec<u8>> = vec![];

        let test_content = "Une nouvelle phrase test d'un certain nombre de bytes, je ne sais  Lorem ipsum dolor sit amet, officia excepteur ex fugiat reprehenderit enim labore culpa sint ad nisi Lorem pariatur mollit ex esse exercitation amet. Nisi anim cupidatat excepteur officia. Reprehenderit nostrud nostrud ipsum Lorem est aliquip amet voluptate voluptate dolor minim nulla est proident. Nostrud officia pariatur ut officia. Sit irure elit esse ea nulla sunt ex occaecat reprehenderit commodo officia dolor Lorem duis laboris cupidatat officia voluptate. Culpa proident adipisicing id nulla nisi laboris ex in Lorem sunt duis officia eiusmod. Aliqua reprehenderit commodo ex non excepteur duis sunt velit enim. Voluptate laboris sint cupidatat ullamco ut ea consectetur et est culpa et culpa duis.".to_string();

        assert!(test_content.len() * messages_quantity > packet_size);
        assert!(test_content.len() > packet_size);

        for i in 0..messages_quantity {
            let frame = test_content.as_bytes().to_vec().prepend_frame().unwrap();

            messages.push(frame);
        }

        // messages contenation to get one vec!

        let concatened = messages.concat();

        assert!(concatened.len() == (test_content.as_bytes().len() + HDR_SIZE) * messages_quantity);

        // split the concatenated vec in chunks of packet's size , to simulate packet streaming

        let mut stream_chunks: Vec<Vec<u8>> = vec![];

        for chunk in concatened.chunks(packet_size) {
            stream_chunks.push(chunk.to_vec());
        }

        /*
        assert!(
            stream_chunks.len() == (concatened.len() as f32 / packet_size as f32).ceil() as usize
        );*/

        let stream_channel = crossbeam::channel::unbounded::<Vec<u8>>();

        let sender = stream_channel.0.clone();
        let receiver = stream_channel.1.clone();

        // create a stream sender
        //
        let len = stream_chunks.len();

        std::thread::spawn(move || {
            for packet in stream_chunks {
                let _ = sender.send(packet);
            }
        });

        // client simulation :

        let thread_handle = std::thread::spawn(move || {
            let mut previous_incompleted_data: Option<(usize, Vec<u8>)> = None;
            let mut truncated_header: Option<Vec<u8>> = None;

            let mut messages_received: Vec<String> = vec![];
            let mut loop_count = 0;
            while let Ok(packet) = receiver.recv() {
                loop_count += 1;
                let parsed = match packet
                    .parse_frame_header(previous_incompleted_data.take(), truncated_header.take())
                {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        println!("er [{e:?}]");
                        vec![]
                    }
                };

                for p in parsed {
                    match p {
                        ParsedStreamData::Completed(data) => {
                            messages_received.push(String::from_utf8_lossy(&data).to_string());
                        }
                        ParsedStreamData::Incompleted(message_size, data) => {
                            previous_incompleted_data = Some((message_size, data));
                        }
                        ParsedStreamData::TruncatedHeader(truncadted_hdr) => {
                            truncated_header = Some(truncadted_hdr);
                        }
                    }
                }
                if loop_count == len {
                    break;
                }
            }

            println!(
                "number messages[{:?}] / [{}]",
                messages_quantity,
                messages_received.len()
            );
            assert!(messages_received.len() == messages_quantity);
        });

        assert!(thread_handle.join().is_ok());
    }
    #[test]
    fn send_messages_larger_than_packet_size_with_very_tiny_packet_inferior_to_hdr_size() {
        let packet_size = HDR_SIZE - 1; //

        let messages_quantity = 10;
        let mut messages: Vec<Vec<u8>> = vec![];

        let test_content = "Une nouvelle phrase test d'un certain nombre de bytes, je ne sais  Lorem ipsum dolor sit amet, officia excepteur ex fugiat reprehenderit enim labore culpa sint ad nisi Lorem pariatur mollit ex esse exercitation amet. Nisi anim cupidatat excepteur officia. Reprehenderit nostrud nostrud ipsum Lorem est aliquip amet voluptate voluptate dolor minim nulla est proident. Nostrud officia pariatur ut officia. Sit irure elit esse ea nulla sunt ex occaecat reprehenderit commodo officia dolor Lorem duis laboris cupidatat officia voluptate. Culpa proident adipisicing id nulla nisi laboris ex in Lorem sunt duis officia eiusmod. Aliqua reprehenderit commodo ex non excepteur duis sunt velit enim. Voluptate laboris sint cupidatat ullamco ut ea consectetur et est culpa et culpa duis.".to_string();
        println!("[{:?}]", &test_content.as_bytes()[..20]);

        assert!(test_content.len() * messages_quantity > packet_size);
        assert!(test_content.len() > packet_size);

        for i in 0..messages_quantity {
            let frame = test_content.as_bytes().to_vec().prepend_frame().unwrap();

            messages.push(frame);
        }

        // messages contenation to get one vec!

        let concatened = messages.concat();

        assert!(concatened.len() == (test_content.as_bytes().len() + HDR_SIZE) * messages_quantity);

        // split the concatenated vec in chunks of packet's size , to simulate packet streaming

        let mut stream_chunks: Vec<Vec<u8>> = vec![];

        for chunk in concatened.chunks(packet_size) {
            stream_chunks.push(chunk.to_vec());
        }

        println!("number of packe [{:?}]", stream_chunks.len());
        println!("packet len[{:?}]", stream_chunks[0].len());

        /*
        assert!(
            stream_chunks.len() == (concatened.len() as f32 / packet_size as f32).ceil() as usize
        );*/

        let stream_channel = crossbeam::channel::unbounded::<Vec<u8>>();

        let sender = stream_channel.0.clone();
        let receiver = stream_channel.1.clone();

        // create a stream sender
        //
        let len = stream_chunks.len();

        std::thread::spawn(move || {
            for packet in stream_chunks {
                let _ = sender.send(packet);
            }
        });

        // client simulation :

        let thread_handle = std::thread::spawn(move || {
            let mut previous_incompleted_data: Option<(usize, Vec<u8>)> = None;
            let mut truncated_header: Option<Vec<u8>> = None;

            let mut messages_received: Vec<String> = vec![];
            let mut loop_count = 0;
            while let Ok(packet) = receiver.recv() {
                loop_count += 1;
                if loop_count == len {
                    /*
                                        println!("[{:?}]", truncated_header);
                                        println!("[{:?}]", previous_incompleted_data);
                                        println!("[{:?}]", &packet);
                                        println!("[{:?}]", String::from_utf8(packet.clone()));
                    */
                }
                let parsed = match packet
                    .parse_frame_header(previous_incompleted_data.take(), truncated_header.take())
                {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        println!("er [{e:?}]");
                        vec![]
                    }
                };

                for p in parsed {
                    match p {
                        ParsedStreamData::Completed(data) => {
                            messages_received.push(String::from_utf8_lossy(&data).to_string());
                        }
                        ParsedStreamData::Incompleted(message_size, data) => {
                            previous_incompleted_data = Some((message_size, data));
                        }
                        ParsedStreamData::TruncatedHeader(truncadted_hdr) => {
                            truncated_header = Some(truncadted_hdr);
                        }
                    }
                }
                if loop_count == len {
                    break;
                }
            }

            println!(
                "number messages[{:?}] / [{}]",
                messages_quantity,
                messages_received.len()
            );
            assert!(messages_received.len() == messages_quantity);
        });

        assert!(thread_handle.join().is_ok());
    }
    #[test]
    fn send_messages_larger_than_packet_size_with_very_tiny_packet_inferior_to_hdr_size_2() {
        let packet_size = HDR_SIZE - 2; //

        let messages_quantity = 1000;
        let mut messages: Vec<Vec<u8>> = vec![];

        let test_content = "Une nouvelle phrase test d'un certain nombre de bytes, je ne sais  Lorem ipsum dolor sit amet, officia excepteur ex fugiat reprehenderit enim labore culpa sint ad nisi Lorem pariatur mollit ex esse exercitation amet. Nisi anim cupidatat excepteur officia. Reprehenderit nostrud nostrud ipsum Lorem est aliquip amet voluptate voluptate dolor minim nulla est proident. Nostrud officia pariatur ut officia. Sit irure elit esse ea nulla sunt ex occaecat reprehenderit commodo officia dolor Lorem duis laboris cupidatat officia voluptate. Culpa proident adipisicing id nulla nisi laboris ex in Lorem sunt duis officia eiusmod. Aliqua reprehenderit commodo ex non excepteur duis sunt velit enim. Voluptate laboris sint cupidatat ullamco ut ea consectetur et est culpa et culpa duis.".to_string();
        println!("[{:?}]", &test_content.as_bytes()[..20]);

        assert!(test_content.len() * messages_quantity > packet_size);
        assert!(test_content.len() > packet_size);

        for i in 0..messages_quantity {
            let frame = test_content.as_bytes().to_vec().prepend_frame().unwrap();

            messages.push(frame);
        }

        // messages contenation to get one vec!

        let concatened = messages.concat();

        assert!(concatened.len() == (test_content.as_bytes().len() + HDR_SIZE) * messages_quantity);

        // split the concatenated vec in chunks of packet's size , to simulate packet streaming

        let mut stream_chunks: Vec<Vec<u8>> = vec![];

        for chunk in concatened.chunks(packet_size) {
            stream_chunks.push(chunk.to_vec());
        }

        println!("number of packe [{:?}]", stream_chunks.len());
        println!("packet len[{:?}]", stream_chunks[0].len());

        /*
        assert!(
            stream_chunks.len() == (concatened.len() as f32 / packet_size as f32).ceil() as usize
        );*/

        let stream_channel = crossbeam::channel::unbounded::<Vec<u8>>();

        let sender = stream_channel.0.clone();
        let receiver = stream_channel.1.clone();

        // create a stream sender
        //
        let len = stream_chunks.len();

        std::thread::spawn(move || {
            for packet in stream_chunks {
                let _ = sender.send(packet);
            }
        });

        // client simulation :

        let thread_handle = std::thread::spawn(move || {
            let mut previous_incompleted_data: Option<(usize, Vec<u8>)> = None;
            let mut truncated_header: Option<Vec<u8>> = None;

            let mut messages_received: Vec<String> = vec![];
            let mut loop_count = 0;
            while let Ok(packet) = receiver.recv() {
                loop_count += 1;
                if loop_count == len {
                    /*
                                        println!("[{:?}]", truncated_header);
                                        println!("[{:?}]", previous_incompleted_data);
                                        println!("[{:?}]", &packet);
                                        println!("[{:?}]", String::from_utf8(packet.clone()));
                    */
                }
                let parsed = match packet
                    .parse_frame_header(previous_incompleted_data.take(), truncated_header.take())
                {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        println!("er [{e:?}]");
                        vec![]
                    }
                };

                for p in parsed {
                    match p {
                        ParsedStreamData::Completed(data) => {
                            messages_received.push(String::from_utf8_lossy(&data).to_string());
                        }
                        ParsedStreamData::Incompleted(message_size, data) => {
                            previous_incompleted_data = Some((message_size, data));
                        }
                        ParsedStreamData::TruncatedHeader(truncadted_hdr) => {
                            truncated_header = Some(truncadted_hdr);
                        }
                    }
                }
                if loop_count == len {
                    break;
                }
            }

            println!(
                "number messages[{:?}] / [{}]",
                messages_quantity,
                messages_received.len()
            );
            assert!(messages_received.len() == messages_quantity);
        });

        assert!(thread_handle.join().is_ok());
    }
    #[test]
    fn send_messages_larger_than_packet_size_with_very_tiny_packet_inferior_to_hdr_size_3() {
        let packet_size = HDR_SIZE - 3; // 

        let messages_quantity = 1001;
        let mut messages: Vec<Vec<u8>> = vec![];

        let test_content = "Une nouvelle phrase test d'un certain nombre de bytes, je ne sais  Lorem ipsum dolor sit amet, officia excepteur ex fugiat reprehenderit enim labore culpa sint ad nisi Lorem pariatur mollit ex esse exercitation amet. Nisi anim cupidatat excepteur officia. Reprehenderit nostrud nostrud ipsum Lorem est aliquip amet voluptate voluptate dolor minim nulla est proident. Nostrud officia pariatur ut officia. Sit irure elit esse ea nulla sunt ex occaecat reprehenderit commodo officia dolor Lorem duis laboris cupidatat officia voluptate. Culpa proident adipisicing id nulla nisi laboris ex in Lorem sunt duis officia eiusmod. Aliqua reprehenderit commodo ex non excepteur duis sunt velit enim. Voluptate laboris sint cupidatat ullamco ut ea consectetur et est culpa et culpa duis.".to_string();
        println!("[{:?}]", &test_content.as_bytes()[..20]);

        assert!(test_content.len() * messages_quantity > packet_size);
        assert!(test_content.len() > packet_size);

        for i in 0..messages_quantity {
            let frame = test_content.as_bytes().to_vec().prepend_frame().unwrap();

            messages.push(frame);
        }

        // messages contenation to get one vec!

        let concatened = messages.concat();

        assert!(concatened.len() == (test_content.as_bytes().len() + HDR_SIZE) * messages_quantity);

        // split the concatenated vec in chunks of packet's size , to simulate packet streaming

        let mut stream_chunks: Vec<Vec<u8>> = vec![];

        for chunk in concatened.chunks(packet_size) {
            stream_chunks.push(chunk.to_vec());
        }

        println!("number of packe [{:?}]", stream_chunks.len());
        println!("packet len[{:?}]", stream_chunks[0].len());

        /*
        assert!(
            stream_chunks.len() == (concatened.len() as f32 / packet_size as f32).ceil() as usize
        );*/

        let stream_channel = crossbeam::channel::unbounded::<Vec<u8>>();

        let sender = stream_channel.0.clone();
        let receiver = stream_channel.1.clone();

        // create a stream sender
        //
        let len = stream_chunks.len();

        std::thread::spawn(move || {
            for packet in stream_chunks {
                let _ = sender.send(packet);
            }
        });

        // client simulation :

        let thread_handle = std::thread::spawn(move || {
            let mut previous_incompleted_data: Option<(usize, Vec<u8>)> = None;
            let mut truncated_header: Option<Vec<u8>> = None;

            let mut messages_received: Vec<String> = vec![];
            let mut loop_count = 0;
            while let Ok(packet) = receiver.recv() {
                loop_count += 1;
                if loop_count == len {
                    /*
                                        println!("[{:?}]", truncated_header);
                                        println!("[{:?}]", previous_incompleted_data);
                                        println!("[{:?}]", &packet);
                                        println!("[{:?}]", String::from_utf8(packet.clone()));
                    */
                }
                let parsed = match packet
                    .parse_frame_header(previous_incompleted_data.take(), truncated_header.take())
                {
                    Ok(parsed) => parsed,

                    Err(e) => {
                        println!("er [{e:?}]");
                        vec![]
                    }
                };

                for p in parsed {
                    match p {
                        ParsedStreamData::Completed(data) => {
                            messages_received.push(String::from_utf8_lossy(&data).to_string());
                        }
                        ParsedStreamData::Incompleted(message_size, data) => {
                            previous_incompleted_data = Some((message_size, data));
                        }
                        ParsedStreamData::TruncatedHeader(truncadted_hdr) => {
                            truncated_header = Some(truncadted_hdr);
                        }
                    }
                }
                if loop_count == len {
                    break;
                }
            }

            println!(
                "number messages[{:?}] / [{}]",
                messages_quantity,
                messages_received.len()
            );
            assert!(messages_received.len() == messages_quantity);
        });

        assert!(thread_handle.join().is_ok());
    }
    #[test]
    fn send_messages_larger_than_packet_size_with_very_tiny_packet_inferior_to_hdr_size_4() {
        let packet_size = HDR_SIZE - 4; // 

        let messages_quantity = 1001;
        let mut messages: Vec<Vec<u8>> = vec![];

        let test_content = "Une nouvelle phrase test d'un certain nombre de bytes, je ne sais  Lorem ipsum dolor sit amet, officia excepteur ex fugiat reprehenderit enim labore culpa sint ad nisi Lorem pariatur mollit ex esse exercitation amet. Nisi anim cupidatat excepteur officia. Reprehenderit nostrud nostrud ipsum Lorem est aliquip amet voluptate voluptate dolor minim nulla est proident. Nostrud officia pariatur ut officia. Sit irure elit esse ea nulla sunt ex occaecat reprehenderit commodo officia dolor Lorem duis laboris cupidatat officia voluptate. Culpa proident adipisicing id nulla nisi laboris ex in Lorem sunt duis officia eiusmod. Aliqua reprehenderit commodo ex non excepteur duis sunt velit enim. Voluptate laboris sint cupidatat ullamco ut ea consectetur et est culpa et culpa duis.".to_string();
        println!("[{:?}]", &test_content.as_bytes()[..20]);

        assert!(test_content.len() * messages_quantity > packet_size);
        assert!(test_content.len() > packet_size);

        for i in 0..messages_quantity {
            let frame = test_content.as_bytes().to_vec().prepend_frame().unwrap();

            messages.push(frame);
        }

        // messages contenation to get one vec!

        let concatened = messages.concat();

        assert!(concatened.len() == (test_content.as_bytes().len() + HDR_SIZE) * messages_quantity);

        // split the concatenated vec in chunks of packet's size , to simulate packet streaming

        let mut stream_chunks: Vec<Vec<u8>> = vec![];

        for chunk in concatened.chunks(packet_size) {
            stream_chunks.push(chunk.to_vec());
        }

        println!("number of packe [{:?}]", stream_chunks.len());
        println!("packet len[{:?}]", stream_chunks[0].len());

        /*
        assert!(
            stream_chunks.len() == (concatened.len() as f32 / packet_size as f32).ceil() as usize
        );*/

        let stream_channel = crossbeam::channel::unbounded::<Vec<u8>>();

        let sender = stream_channel.0.clone();
        let receiver = stream_channel.1.clone();

        // create a stream sender
        //
        let len = stream_chunks.len();

        std::thread::spawn(move || {
            for packet in stream_chunks {
                let _ = sender.send(packet);
            }
        });

        // client simulation :

        let thread_handle = std::thread::spawn(move || {
            let mut previous_incompleted_data: Option<(usize, Vec<u8>)> = None;
            let mut truncated_header: Option<Vec<u8>> = None;

            let mut messages_received: Vec<String> = vec![];
            let mut loop_count = 0;
            while let Ok(packet) = receiver.recv() {
                loop_count += 1;
                if loop_count == len {
                    /*
                                        println!("[{:?}]", truncated_header);
                                        println!("[{:?}]", previous_incompleted_data);
                                        println!("[{:?}]", &packet);
                                        println!("[{:?}]", String::from_utf8(packet.clone()));
                    */
                }
                let parsed = match packet
                    .parse_frame_header(previous_incompleted_data.take(), truncated_header.take())
                {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        println!("er [{e:?}]");
                        vec![]
                    }
                };

                for p in parsed {
                    match p {
                        ParsedStreamData::Completed(data) => {
                            messages_received.push(String::from_utf8_lossy(&data).to_string());
                        }
                        ParsedStreamData::Incompleted(message_size, data) => {
                            previous_incompleted_data = Some((message_size, data));
                        }
                        ParsedStreamData::TruncatedHeader(truncadted_hdr) => {
                            truncated_header = Some(truncadted_hdr);
                        }
                    }
                }
                if loop_count == len {
                    break;
                }
            }

            println!(
                "number messages[{:?}] / [{}]",
                messages_quantity,
                messages_received.len()
            );
            assert!(messages_received.len() == messages_quantity);
        });

        assert!(thread_handle.join().is_ok());
    }
    #[test]
    fn send_messages_larger_than_packet_size_with_very_tiny_packet_inferior_to_hdr_size_5() {
        let packet_size = HDR_SIZE - 5; // 

        let messages_quantity = 1001;
        let mut messages: Vec<Vec<u8>> = vec![];

        let test_content = "Une nouvelle phrase test d'un certain nombre de bytes, je ne sais  Lorem ipsum dolor sit amet, officia excepteur ex fugiat reprehenderit enim labore culpa sint ad nisi Lorem pariatur mollit ex esse exercitation amet. Nisi anim cupidatat excepteur officia. Reprehenderit nostrud nostrud ipsum Lorem est aliquip amet voluptate voluptate dolor minim nulla est proident. Nostrud officia pariatur ut officia. Sit irure elit esse ea nulla sunt ex occaecat reprehenderit commodo officia dolor Lorem duis laboris cupidatat officia voluptate. Culpa proident adipisicing id nulla nisi laboris ex in Lorem sunt duis officia eiusmod. Aliqua reprehenderit commodo ex non excepteur duis sunt velit enim. Voluptate laboris sint cupidatat ullamco ut ea consectetur et est culpa et culpa duis.".to_string();
        println!("[{:?}]", &test_content.as_bytes()[..20]);

        assert!(test_content.len() * messages_quantity > packet_size);
        assert!(test_content.len() > packet_size);

        for i in 0..messages_quantity {
            let frame = test_content.as_bytes().to_vec().prepend_frame().unwrap();

            messages.push(frame);
        }

        // messages contenation to get one vec!

        let concatened = messages.concat();

        assert!(concatened.len() == (test_content.as_bytes().len() + HDR_SIZE) * messages_quantity);

        // split the concatenated vec in chunks of packet's size , to simulate packet streaming

        let mut stream_chunks: Vec<Vec<u8>> = vec![];

        for chunk in concatened.chunks(packet_size) {
            stream_chunks.push(chunk.to_vec());
        }

        println!("number of packe [{:?}]", stream_chunks.len());
        println!("packet len[{:?}]", stream_chunks[0].len());

        /*
        assert!(
            stream_chunks.len() == (concatened.len() as f32 / packet_size as f32).ceil() as usize
        );*/

        let stream_channel = crossbeam::channel::unbounded::<Vec<u8>>();

        let sender = stream_channel.0.clone();
        let receiver = stream_channel.1.clone();

        // create a stream sender
        //
        let len = stream_chunks.len();

        std::thread::spawn(move || {
            for packet in stream_chunks {
                let _ = sender.send(packet);
            }
        });

        // client simulation :

        let thread_handle = std::thread::spawn(move || {
            let mut previous_incompleted_data: Option<(usize, Vec<u8>)> = None;
            let mut truncated_header: Option<Vec<u8>> = None;

            let mut messages_received: Vec<String> = vec![];
            let mut loop_count = 0;
            while let Ok(packet) = receiver.recv() {
                loop_count += 1;
                if loop_count == len {
                    /*
                                        println!("[{:?}]", truncated_header);
                                        println!("[{:?}]", previous_incompleted_data);
                                        println!("[{:?}]", &packet);
                                        println!("[{:?}]", String::from_utf8(packet.clone()));
                    */
                }
                let parsed = match packet
                    .parse_frame_header(previous_incompleted_data.take(), truncated_header.take())
                {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        println!("er [{e:?}]");
                        vec![]
                    }
                };

                for p in parsed {
                    match p {
                        ParsedStreamData::Completed(data) => {
                            messages_received.push(String::from_utf8_lossy(&data).to_string());
                        }
                        ParsedStreamData::Incompleted(message_size, data) => {
                            previous_incompleted_data = Some((message_size, data));
                        }
                        ParsedStreamData::TruncatedHeader(truncadted_hdr) => {
                            truncated_header = Some(truncadted_hdr);
                        }
                    }
                }
                if loop_count == len {
                    break;
                }
            }

            println!(
                "number messages[{:?}] / [{}]",
                messages_quantity,
                messages_received.len()
            );
            assert!(messages_received.len() == messages_quantity);
        });

        assert!(thread_handle.join().is_ok());
    }
    #[test]
    fn send_messages_larger_than_packet_size_with_very_tiny_packet_inferior_to_hdr_size_6() {
        let packet_size = HDR_SIZE - 6; // 

        let messages_quantity = 1001;
        let mut messages: Vec<Vec<u8>> = vec![];

        let test_content = "Une nouvelle phrase test d'un certain nombre de bytes, je ne sais  Lorem ipsum dolor sit amet, officia excepteur ex fugiat reprehenderit enim labore culpa sint ad nisi Lorem pariatur mollit ex esse exercitation amet. Nisi anim cupidatat excepteur officia. Reprehenderit nostrud nostrud ipsum Lorem est aliquip amet voluptate voluptate dolor minim nulla est proident. Nostrud officia pariatur ut officia. Sit irure elit esse ea nulla sunt ex occaecat reprehenderit commodo officia dolor Lorem duis laboris cupidatat officia voluptate. Culpa proident adipisicing id nulla nisi laboris ex in Lorem sunt duis officia eiusmod. Aliqua reprehenderit commodo ex non excepteur duis sunt velit enim. Voluptate laboris sint cupidatat ullamco ut ea consectetur et est culpa et culpa duis.".to_string();
        println!("[{:?}]", &test_content.as_bytes()[..20]);

        assert!(test_content.len() * messages_quantity > packet_size);
        assert!(test_content.len() > packet_size);

        for i in 0..messages_quantity {
            let frame = test_content.as_bytes().to_vec().prepend_frame().unwrap();

            messages.push(frame);
        }

        // messages contenation to get one vec!

        let concatened = messages.concat();

        assert!(concatened.len() == (test_content.as_bytes().len() + HDR_SIZE) * messages_quantity);

        // split the concatenated vec in chunks of packet's size , to simulate packet streaming

        let mut stream_chunks: Vec<Vec<u8>> = vec![];

        for chunk in concatened.chunks(packet_size) {
            stream_chunks.push(chunk.to_vec());
        }

        println!("number of packe [{:?}]", stream_chunks.len());
        println!("packet len[{:?}]", stream_chunks[0].len());

        /*
        assert!(
            stream_chunks.len() == (concatened.len() as f32 / packet_size as f32).ceil() as usize
        );*/

        let stream_channel = crossbeam::channel::unbounded::<Vec<u8>>();

        let sender = stream_channel.0.clone();
        let receiver = stream_channel.1.clone();

        // create a stream sender
        //
        let len = stream_chunks.len();

        std::thread::spawn(move || {
            for packet in stream_chunks {
                let _ = sender.send(packet);
            }
        });

        // client simulation :

        let thread_handle = std::thread::spawn(move || {
            let mut previous_incompleted_data: Option<(usize, Vec<u8>)> = None;
            let mut truncated_header: Option<Vec<u8>> = None;

            let mut messages_received: Vec<String> = vec![];
            let mut loop_count = 0;
            while let Ok(packet) = receiver.recv() {
                loop_count += 1;
                if loop_count == len {
                    /*
                                        println!("[{:?}]", truncated_header);
                                        println!("[{:?}]", previous_incompleted_data);
                                        println!("[{:?}]", &packet);
                                        println!("[{:?}]", String::from_utf8(packet.clone()));
                    */
                }
                let parsed = match packet
                    .parse_frame_header(previous_incompleted_data.take(), truncated_header.take())
                {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        println!("er [{e:?}]");
                        vec![]
                    }
                };

                for p in parsed {
                    match p {
                        ParsedStreamData::Completed(data) => {
                            messages_received.push(String::from_utf8_lossy(&data).to_string());
                        }
                        ParsedStreamData::Incompleted(message_size, data) => {
                            previous_incompleted_data = Some((message_size, data));
                        }
                        ParsedStreamData::TruncatedHeader(truncadted_hdr) => {
                            truncated_header = Some(truncadted_hdr);
                        }
                    }
                }
                if loop_count == len {
                    break;
                }
            }

            println!(
                "number messages[{:?}] / [{}]",
                messages_quantity,
                messages_received.len()
            );
            assert!(messages_received.len() == messages_quantity);
        });

        assert!(thread_handle.join().is_ok());
    }
    #[test]
    fn send_messages_larger_than_packet_size_with_very_tiny_packet_inferior_to_hdr_size_7() {
        let packet_size = HDR_SIZE - 7; // 

        let messages_quantity = 1001;
        let mut messages: Vec<Vec<u8>> = vec![];

        let test_content = "Une nouvelle phrase test d'un certain nombre de bytes, je ne sais  Lorem ipsum dolor sit amet, officia excepteur ex fugiat reprehenderit enim labore culpa sint ad nisi Lorem pariatur mollit ex esse exercitation amet. Nisi anim cupidatat excepteur officia. Reprehenderit nostrud nostrud ipsum Lorem est aliquip amet voluptate voluptate dolor minim nulla est proident. Nostrud officia pariatur ut officia. Sit irure elit esse ea nulla sunt ex occaecat reprehenderit commodo officia dolor Lorem duis laboris cupidatat officia voluptate. Culpa proident adipisicing id nulla nisi laboris ex in Lorem sunt duis officia eiusmod. Aliqua reprehenderit commodo ex non excepteur duis sunt velit enim. Voluptate laboris sint cupidatat ullamco ut ea consectetur et est culpa et culpa duis.".to_string();
        println!("[{:?}]", &test_content.as_bytes()[..20]);

        assert!(test_content.len() * messages_quantity > packet_size);
        assert!(test_content.len() > packet_size);

        for i in 0..messages_quantity {
            let frame = test_content.as_bytes().to_vec().prepend_frame().unwrap();

            messages.push(frame);
        }

        // messages contenation to get one vec!

        let concatened = messages.concat();

        assert!(concatened.len() == (test_content.as_bytes().len() + HDR_SIZE) * messages_quantity);

        // split the concatenated vec in chunks of packet's size , to simulate packet streaming

        let mut stream_chunks: Vec<Vec<u8>> = vec![];

        for chunk in concatened.chunks(packet_size) {
            stream_chunks.push(chunk.to_vec());
        }

        println!("number of packe [{:?}]", stream_chunks.len());
        println!("packet len[{:?}]", stream_chunks[0].len());

        /*
        assert!(
            stream_chunks.len() == (concatened.len() as f32 / packet_size as f32).ceil() as usize
        );*/

        let stream_channel = crossbeam::channel::unbounded::<Vec<u8>>();

        let sender = stream_channel.0.clone();
        let receiver = stream_channel.1.clone();

        // create a stream sender
        //
        let len = stream_chunks.len();

        std::thread::spawn(move || {
            for packet in stream_chunks {
                let _ = sender.send(packet);
            }
        });

        // client simulation :

        let thread_handle = std::thread::spawn(move || {
            let mut previous_incompleted_data: Option<(usize, Vec<u8>)> = None;
            let mut truncated_header: Option<Vec<u8>> = None;

            let mut messages_received: Vec<String> = vec![];
            let mut loop_count = 0;
            while let Ok(packet) = receiver.recv() {
                loop_count += 1;
                if loop_count == len {
                    /*
                                        println!("[{:?}]", truncated_header);
                                        println!("[{:?}]", previous_incompleted_data);
                                        println!("[{:?}]", &packet);
                                        println!("[{:?}]", String::from_utf8(packet.clone()));
                    */
                }
                let parsed = match packet
                    .parse_frame_header(previous_incompleted_data.take(), truncated_header.take())
                {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        println!("er [{e:?}]");
                        vec![]
                    }
                };

                for p in parsed {
                    match p {
                        ParsedStreamData::Completed(data) => {
                            messages_received.push(String::from_utf8_lossy(&data).to_string());
                        }
                        ParsedStreamData::Incompleted(message_size, data) => {
                            previous_incompleted_data = Some((message_size, data));
                        }
                        ParsedStreamData::TruncatedHeader(truncadted_hdr) => {
                            truncated_header = Some(truncadted_hdr);
                        }
                    }
                }
                if loop_count == len {
                    break;
                }
            }

            println!(
                "number messages[{:?}] / [{}]",
                messages_quantity,
                messages_received.len()
            );
            assert!(messages_received.len() == messages_quantity);
        });

        assert!(thread_handle.join().is_ok());
    }
    #[test]
    fn send_messages_larger_than_packet_size_with_very_tiny_packet_inferior_to_hdr_size_8() {
        let packet_size = HDR_SIZE - 8; // 

        let messages_quantity = 1003;
        let mut messages: Vec<Vec<u8>> = vec![];

        let test_content = "Une nouvelle phrase test d'un certain nombre de bytes, je ne sais  Lorem ipsum dolor sit amet, officia excepteur ex fugiat reprehenderit enim labore culpa sint ad nisi Lorem pariatur mollit ex esse exercitation amet. Nisi anim cupidatat excepteur officia. Reprehenderit nostrud nostrud ipsum Lorem est aliquip amet voluptate voluptate dolor minim nulla est proident. Nostrud officia pariatur ut officia. Sit irure elit esse ea nulla sunt ex occaecat reprehenderit commodo officia dolor Lorem duis laboris cupidatat officia voluptate. Culpa proident adipisicing id nulla nisi laboris ex in Lorem sunt duis officia eiusmod. Aliqua reprehenderit commodo ex non excepteur duis sunt velit enim. Voluptate laboris sint cupidatat ullamco ut ea consectetur et est culpa et culpa duis.".to_string();
        println!("[{:?}]", &test_content.as_bytes()[..20]);

        assert!(test_content.len() * messages_quantity > packet_size);
        assert!(test_content.len() > packet_size);

        for i in 0..messages_quantity {
            let frame = test_content.as_bytes().to_vec().prepend_frame().unwrap();

            messages.push(frame);
        }

        // messages contenation to get one vec!

        let concatened = messages.concat();

        assert!(concatened.len() == (test_content.as_bytes().len() + HDR_SIZE) * messages_quantity);

        // split the concatenated vec in chunks of packet's size , to simulate packet streaming

        let mut stream_chunks: Vec<Vec<u8>> = vec![];

        for chunk in concatened.chunks(packet_size) {
            stream_chunks.push(chunk.to_vec());
        }

        println!("number of packe [{:?}]", stream_chunks.len());
        println!("packet len[{:?}]", stream_chunks[0].len());

        /*
        assert!(
            stream_chunks.len() == (concatened.len() as f32 / packet_size as f32).ceil() as usize
        );*/

        let stream_channel = crossbeam::channel::unbounded::<Vec<u8>>();

        let sender = stream_channel.0.clone();
        let receiver = stream_channel.1.clone();

        // create a stream sender
        //
        let len = stream_chunks.len();

        std::thread::spawn(move || {
            for packet in stream_chunks {
                let _ = sender.send(packet);
            }
        });

        // client simulation :

        let thread_handle = std::thread::spawn(move || {
            let mut previous_incompleted_data: Option<(usize, Vec<u8>)> = None;
            let mut truncated_header: Option<Vec<u8>> = None;

            let mut messages_received: Vec<String> = vec![];
            let mut loop_count = 0;
            while let Ok(packet) = receiver.recv() {
                loop_count += 1;
                if loop_count == len {
                    /*
                                        println!("[{:?}]", truncated_header);
                                        println!("[{:?}]", previous_incompleted_data);
                                        println!("[{:?}]", &packet);
                                        println!("[{:?}]", String::from_utf8(packet.clone()));
                    */
                }
                let parsed = match packet
                    .parse_frame_header(previous_incompleted_data.take(), truncated_header.take())
                {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        println!("er [{e:?}]");
                        vec![]
                    }
                };

                for p in parsed {
                    match p {
                        ParsedStreamData::Completed(data) => {
                            messages_received.push(String::from_utf8_lossy(&data).to_string());
                        }
                        ParsedStreamData::Incompleted(message_size, data) => {
                            previous_incompleted_data = Some((message_size, data));
                        }
                        ParsedStreamData::TruncatedHeader(truncadted_hdr) => {
                            truncated_header = Some(truncadted_hdr);
                        }
                    }
                }
                if loop_count == len {
                    break;
                }
            }

            println!(
                "number messages[{:?}] / [{}]",
                messages_quantity,
                messages_received.len()
            );
            assert!(messages_received.len() == messages_quantity);
        });

        assert!(thread_handle.join().is_ok());
    }
    #[test]
    fn send_messages_larger_than_packet_size_with_very_tiny_packet_inferior_to_hdr_size_10() {
        let packet_size = HDR_SIZE - 10; // 

        let messages_quantity = 1003;
        let mut messages: Vec<Vec<u8>> = vec![];

        let test_content = "Une nouvelle phrase test d'un certain nombre de bytes, je ne sais  Lorem ipsum dolor sit amet, officia excepteur ex fugiat reprehenderit enim labore culpa sint ad nisi Lorem pariatur mollit ex esse exercitation amet. Nisi anim cupidatat excepteur officia. Reprehenderit nostrud nostrud ipsum Lorem est aliquip amet voluptate voluptate dolor minim nulla est proident. Nostrud officia pariatur ut officia. Sit irure elit esse ea nulla sunt ex occaecat reprehenderit commodo officia dolor Lorem duis laboris cupidatat officia voluptate. Culpa proident adipisicing id nulla nisi laboris ex in Lorem sunt duis officia eiusmod. Aliqua reprehenderit commodo ex non excepteur duis sunt velit enim. Voluptate laboris sint cupidatat ullamco ut ea consectetur et est culpa et culpa duis.".to_string();
        println!("[{:?}]", &test_content.as_bytes()[..20]);

        assert!(test_content.len() * messages_quantity > packet_size);
        assert!(test_content.len() > packet_size);

        for i in 0..messages_quantity {
            let frame = test_content.as_bytes().to_vec().prepend_frame().unwrap();

            messages.push(frame);
        }

        // messages contenation to get one vec!

        let concatened = messages.concat();

        assert!(concatened.len() == (test_content.as_bytes().len() + HDR_SIZE) * messages_quantity);

        // split the concatenated vec in chunks of packet's size , to simulate packet streaming

        let mut stream_chunks: Vec<Vec<u8>> = vec![];

        for chunk in concatened.chunks(packet_size) {
            stream_chunks.push(chunk.to_vec());
        }

        println!("number of packe [{:?}]", stream_chunks.len());
        println!("packet len[{:?}]", stream_chunks[0].len());

        /*
        assert!(
            stream_chunks.len() == (concatened.len() as f32 / packet_size as f32).ceil() as usize
        );*/

        let stream_channel = crossbeam::channel::unbounded::<Vec<u8>>();

        let sender = stream_channel.0.clone();
        let receiver = stream_channel.1.clone();

        // create a stream sender
        //
        let len = stream_chunks.len();

        std::thread::spawn(move || {
            for packet in stream_chunks {
                let _ = sender.send(packet);
            }
        });

        // client simulation :

        let thread_handle = std::thread::spawn(move || {
            let mut previous_incompleted_data: Option<(usize, Vec<u8>)> = None;
            let mut truncated_header: Option<Vec<u8>> = None;

            let mut messages_received: Vec<String> = vec![];
            let mut loop_count = 0;
            while let Ok(packet) = receiver.recv() {
                loop_count += 1;
                if loop_count == len {
                    /*
                                        println!("[{:?}]", truncated_header);
                                        println!("[{:?}]", previous_incompleted_data);
                                        println!("[{:?}]", &packet);
                                        println!("[{:?}]", String::from_utf8(packet.clone()));
                    */
                }
                let parsed = match packet
                    .parse_frame_header(previous_incompleted_data.take(), truncated_header.take())
                {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        println!("er [{e:?}]");
                        vec![]
                    }
                };

                for p in parsed {
                    match p {
                        ParsedStreamData::Completed(data) => {
                            messages_received.push(String::from_utf8_lossy(&data).to_string());
                        }
                        ParsedStreamData::Incompleted(message_size, data) => {
                            previous_incompleted_data = Some((message_size, data));
                        }
                        ParsedStreamData::TruncatedHeader(truncadted_hdr) => {
                            truncated_header = Some(truncadted_hdr);
                        }
                    }
                }
                if loop_count == len {
                    break;
                }
            }

            println!(
                "number messages[{:?}] / [{}]",
                messages_quantity,
                messages_received.len()
            );
            assert!(messages_received.len() == messages_quantity);
        });

        assert!(thread_handle.join().is_ok());
    }
    #[test]
    fn send_messages_larger_than_packet_size_with_very_tiny_packet_inferior_to_hdr_size_11() {
        let packet_size = HDR_SIZE - 11; // 

        let messages_quantity = 1013;
        let mut messages: Vec<Vec<u8>> = vec![];

        let test_content = "Une nouvelle phrase test d'un certain nombre de bytes, je ne sais  Lorem ipsum dolor sit amet, officia excepteur ex fugiat reprehenderit enim labore culpa sint ad nisi Lorem pariatur mollit ex esse exercitation amet. Nisi anim cupidatat excepteur officia. Reprehenderit nostrud nostrud ipsum Lorem est aliquip amet voluptate voluptate dolor minim nulla est proident. Nostrud officia pariatur ut officia. Sit irure elit esse ea nulla sunt ex occaecat reprehenderit commodo officia dolor Lorem duis laboris cupidatat officia voluptate. Culpa proident adipisicing id nulla nisi laboris ex in Lorem sunt duis officia eiusmod. Aliqua reprehenderit commodo ex non excepteur duis sunt velit enim. Voluptate laboris sint cupidatat ullamco ut ea consectetur et est culpa et culpa duis.".to_string();
        println!("[{:?}]", &test_content.as_bytes()[..20]);

        assert!(test_content.len() * messages_quantity > packet_size);
        assert!(test_content.len() > packet_size);

        for i in 0..messages_quantity {
            let frame = test_content.as_bytes().to_vec().prepend_frame().unwrap();

            messages.push(frame);
        }

        // messages contenation to get one vec!

        let concatened = messages.concat();

        assert!(concatened.len() == (test_content.as_bytes().len() + HDR_SIZE) * messages_quantity);

        // split the concatenated vec in chunks of packet's size , to simulate packet streaming

        let mut stream_chunks: Vec<Vec<u8>> = vec![];

        for chunk in concatened.chunks(packet_size) {
            stream_chunks.push(chunk.to_vec());
        }

        println!("number of packe [{:?}]", stream_chunks.len());
        println!("packet len[{:?}]", stream_chunks[0].len());

        /*
        assert!(
            stream_chunks.len() == (concatened.len() as f32 / packet_size as f32).ceil() as usize
        );*/

        let stream_channel = crossbeam::channel::unbounded::<Vec<u8>>();

        let sender = stream_channel.0.clone();
        let receiver = stream_channel.1.clone();

        // create a stream sender
        //
        let len = stream_chunks.len();

        std::thread::spawn(move || {
            for packet in stream_chunks {
                let _ = sender.send(packet);
            }
        });

        // client simulation :

        let thread_handle = std::thread::spawn(move || {
            let mut previous_incompleted_data: Option<(usize, Vec<u8>)> = None;
            let mut truncated_header: Option<Vec<u8>> = None;

            let mut messages_received: Vec<String> = vec![];
            let mut loop_count = 0;
            while let Ok(packet) = receiver.recv() {
                loop_count += 1;
                if loop_count == len {
                    /*
                                        println!("[{:?}]", truncated_header);
                                        println!("[{:?}]", previous_incompleted_data);
                                        println!("[{:?}]", &packet);
                                        println!("[{:?}]", String::from_utf8(packet.clone()));
                    */
                }
                let parsed = match packet
                    .parse_frame_header(previous_incompleted_data.take(), truncated_header.take())
                {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        println!("er [{e:?}]");
                        vec![]
                    }
                };

                for p in parsed {
                    match p {
                        ParsedStreamData::Completed(data) => {
                            messages_received.push(String::from_utf8_lossy(&data).to_string());
                        }
                        ParsedStreamData::Incompleted(message_size, data) => {
                            previous_incompleted_data = Some((message_size, data));
                        }
                        ParsedStreamData::TruncatedHeader(truncadted_hdr) => {
                            truncated_header = Some(truncadted_hdr);
                        }
                    }
                }
                if loop_count == len {
                    break;
                }
            }

            println!(
                "number messages[{:?}] / [{}]",
                messages_quantity,
                messages_received.len()
            );
            assert!(messages_received.len() == messages_quantity);
        });

        assert!(thread_handle.join().is_ok());
    }
    #[test]
    fn send_messages_of_zero_len() {
        let packet_size = 128;

        let messages_quantity = 100000;
        let mut messages: Vec<Vec<u8>> = vec![];

        let test_content = "".to_string();

        assert!(test_content.len() == 0);

        for _i in 0..messages_quantity {
            let frame = test_content.as_bytes().to_vec().prepend_frame().unwrap();

            messages.push(frame);
        }

        // messages contenation to get one vec!

        let concatened = messages.concat();

        assert!(concatened.len() == (test_content.as_bytes().len() + HDR_SIZE) * messages_quantity);

        // split the concatenated vec in chunks of packet's size , to simulate packet streaming

        let mut stream_chunks: Vec<Vec<u8>> = vec![];

        for chunk in concatened.chunks(packet_size) {
            stream_chunks.push(chunk.to_vec());
        }

        assert!(
            stream_chunks.len() == (concatened.len() as f32 / packet_size as f32).ceil() as usize
        );

        let stream_channel = crossbeam::channel::unbounded::<Vec<u8>>();

        let sender = stream_channel.0.clone();
        let receiver = stream_channel.1.clone();

        // create a stream sender
        //
        let len = stream_chunks.len();

        std::thread::spawn(move || {
            for packet in stream_chunks {
                let _ = sender.send(packet);
            }
        });

        // client simulation :

        let thread_handle = std::thread::spawn(move || {
            let mut previous_incompleted_data: Option<(usize, Vec<u8>)> = None;
            let mut truncated_header: Option<Vec<u8>> = None;

            let mut messages_received: Vec<String> = vec![];
            let mut loop_count = 0;
            while let Ok(packet) = receiver.recv() {
                loop_count += 1;
                let parsed = match packet
                    .parse_frame_header(previous_incompleted_data.take(), truncated_header.take())
                {
                    Ok(parsed) => parsed,

                    Err(e) => {
                        println!("er [{e:?}]");
                        vec![]
                    }
                };

                for p in parsed {
                    match p {
                        ParsedStreamData::Completed(data) => {
                            messages_received.push(String::from_utf8_lossy(&data).to_string());
                        }
                        ParsedStreamData::Incompleted(message_size, data) => {
                            previous_incompleted_data = Some((message_size, data));
                        }
                        ParsedStreamData::TruncatedHeader(truncadted_hdr) => {
                            truncated_header = Some(truncadted_hdr);
                        }
                    }
                }
                if loop_count == len {
                    break;
                }
            }

            assert!(messages_received.len() == messages_quantity);
        });

        assert!(thread_handle.join().is_ok());
    }
}
