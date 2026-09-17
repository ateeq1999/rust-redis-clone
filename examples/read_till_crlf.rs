use bytes::BytesMut;

fn _read_till_crlf(buf: &[u8]) -> Option<(&[u8], usize)> {
    for (index, window) in buf.windows(2).enumerate() {
        if window == b"\r\n" {
            return Some((&buf[..index], index + 2));
        }
    }
    None
}

const CRLF_BYTES_SIZE: usize = 2;

fn main() {
    let buffer: BytesMut = BytesMut::from("$5\r\nhello\r\n");

    if buffer.len() < 2 {
        panic!("buffer empty")
    } else {
        println!("buffer contains data")
    }

    let identifier = buffer[0];
    let payload_len = buffer[1] as char;
    let window_buffer = &buffer[1..];

    println!("buffer: {:?}", &buffer[..]);
    println!("identifier: {:?}", identifier);
    println!("payload len: {:?}", payload_len);

    let buffer_windows = window_buffer.windows(CRLF_BYTES_SIZE).enumerate();

    for (index, window) in buffer_windows {
        println!("index: {}, and bytes window of 2: {:?}", index, window);

        if window == b"\r\n" {
            println!("index: {} and window: {:?}", index, window);
            println!(
                "&buffer[..{}]: {:?} and index + 2: {}",
                index,
                &window_buffer[..index],
                index + CRLF_BYTES_SIZE
            );

            let data_len: u32 = payload_len.to_string().parse().unwrap();

            println!("payload length: {}", data_len);
            println!(
                "payload start: {}, payload: {:?}",
                index + CRLF_BYTES_SIZE,
                &buffer[index + CRLF_BYTES_SIZE..buffer.len() - CRLF_BYTES_SIZE]
            );
            break;
        }
    }
}
