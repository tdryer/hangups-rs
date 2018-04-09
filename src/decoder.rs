use std::str;

pub struct UnicodeDecoder {
    buffer: Vec<u8>,
}
impl UnicodeDecoder {
    pub fn new() -> Self {
        UnicodeDecoder { buffer: vec![] }
    }

    pub fn push_bytes(&mut self, v: &[u8]) -> String {
        self.buffer.extend_from_slice(v);
        match String::from_utf8(self.buffer.clone()) {
            Ok(s) => {
                self.buffer.clear();
                s
            }
            Err(from_utf8_error) => {
                let e = from_utf8_error.utf8_error();
                match e.error_len() {
                    None => {
                        let valid_up_to = e.valid_up_to();
                        let prefix = self.buffer[..valid_up_to].to_vec();
                        let postfix = self.buffer[valid_up_to..].to_vec();
                        self.buffer = postfix;
                        String::from_utf8(prefix).expect("prefix must be valid")
                    }
                    Some(_) => panic!("unexpected byte encountered"),
                }
            }
        }
    }
}

#[derive(Debug)]
enum ChunkParserState {
    ReadingLength,
    ReadingData {
        expected_length: usize,
        current_length: usize,
    },
}

pub struct ChunkDecoder {
    buffer: String,
    state: ChunkParserState,
}
impl ChunkDecoder {
    pub fn new() -> Self {
        ChunkDecoder {
            buffer: String::new(),
            state: ChunkParserState::ReadingLength,
        }
    }

    fn push_char(&mut self, c: char) -> Option<String> {
        match self.state {
            ChunkParserState::ReadingLength => {
                if c.is_digit(10) {
                    self.buffer.push(c);
                } else if c == '\n' {
                    self.state = ChunkParserState::ReadingData {
                        // TODO: Remove unwrap and handle length being too large.
                        expected_length: self.buffer.parse::<usize>().unwrap(),
                        current_length: 0,
                    };
                    self.buffer.clear();
                } else {
                    // TODO: Convert this to use a Result instead.
                    panic!("invalid chunk length");
                }
                None
            }
            ChunkParserState::ReadingData {
                expected_length,
                mut current_length,
            } => {
                self.buffer.push(c);
                current_length += c.len_utf16();
                if current_length == expected_length {
                    self.state = ChunkParserState::ReadingLength;
                    Some(self.buffer.split_off(0))
                } else {
                    self.state = ChunkParserState::ReadingData {
                        expected_length: expected_length,
                        current_length: current_length,
                    };
                    None
                }
            }
        }
    }

    pub fn push_str(&mut self, new_data: &str) -> Vec<String> {
        new_data.chars().filter_map(|c| self.push_char(c)).collect()
    }
}

#[cfg(test)]
mod tests {

    use decoder;

    #[test]
    fn test_chunk_decoder_simple() {
        let mut chunk_parser = decoder::ChunkDecoder::new();
        assert_eq!(
            chunk_parser.push_str("10\n01234567893\nabc"),
            vec!["0123456789", "abc"]
        );
    }

    #[test]
    fn test_chunk_decoder_incomplete() {
        let mut chunk_parser = decoder::ChunkDecoder::new();
        assert_eq!(
            chunk_parser.push_str("10\n01234567893\nab"),
            vec!["0123456789"]
        );
        assert_eq!(chunk_parser.push_str("c"), vec!["abc"]);
    }

    #[test]
    fn test_chunk_decoder_unicode() {
        let mut chunk_parser = decoder::ChunkDecoder::new();
        assert_eq!(
            // the emoji has length 2
            chunk_parser.push_str("3\na😀"),
            vec!["a😀"]
        );
    }

    #[test]
    fn test_unicode_decoder() {
        let mut unicode_parser = decoder::UnicodeDecoder::new();

        assert_eq!(
            unicode_parser.push_bytes(&String::from("hello").into_bytes()),
            "hello"
        );
        assert_eq!(
            unicode_parser.push_bytes(&String::from("world").into_bytes()),
            "world"
        );

        let smile = String::from("😀").into_bytes();
        assert_eq!(unicode_parser.push_bytes(&[smile[0]]), "");
        assert_eq!(unicode_parser.push_bytes(&[smile[1]]), "");
        assert_eq!(unicode_parser.push_bytes(&[smile[2]]), "");
        assert_eq!(unicode_parser.push_bytes(&[smile[3]]), "😀");

        let mut foo = String::from("hello").into_bytes();
        foo.extend_from_slice(&smile[0..1]);
        assert_eq!(unicode_parser.push_bytes(&foo), "hello");
        assert_eq!(unicode_parser.push_bytes(&smile[1..4]), "😀");
    }

}
