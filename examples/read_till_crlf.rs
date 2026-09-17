use bytes::BytesMut;

fn _read_till_crlf(bytes: &[u8]) -> Option<(&[u8], usize)> {
    for (index, window) in bytes.windows(2).enumerate() {
        if window == b"\r\n" {
            return Some((&bytes[..index], index + 2));
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

    let type_identifier = buffer[0];
    let length_digit_char = buffer[1] as char;
    let bytes_after_identifier = &buffer[1..];

    println!("buffer: {:?}", &buffer[..]);
    println!("identifier: {:?}", type_identifier);
    println!("length digit: {:?}", length_digit_char);

    let sliding_windows = bytes_after_identifier
        .windows(CRLF_BYTES_SIZE)
        .enumerate();

    for (index, window) in sliding_windows {
        println!("index: {}, and bytes window of 2: {:?}", index, window);

        if window == b"\r\n" {
            println!("index: {} and window: {:?}", index, window);
            println!(
                "&buffer[..{}]: {:?} and index + 2: {}",
                index,
                &bytes_after_identifier[..index],
                index + CRLF_BYTES_SIZE
            );

            let bulk_string_length: u32 = length_digit_char.to_string().parse().unwrap();

            println!("payload length: {}", bulk_string_length);
            println!(
                "payload start: {}, payload: {:?}",
                index + CRLF_BYTES_SIZE,
                &buffer[index + CRLF_BYTES_SIZE..buffer.len() - CRLF_BYTES_SIZE]
            );
            break;
        }
    }
}
