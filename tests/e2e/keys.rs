//! Vim/Neovim-style key notation -> terminal input bytes.
//!
//! Tests author input as a single string mixing literal text with `<...>`
//! tokens, e.g. `"abc<Left><Left>X<C-w><Enter>"`. Literal `<` is written
//! `<lt>`. Unknown tokens panic — tests are first-party, so typos should
//! fail loudly rather than be sent to the child as literal text.

/// Translate a key-notation string into write chunks. A bare `<Esc>` ends
/// its chunk: the sender must pause between chunks so a following byte is
/// not coalesced into the same read and misparsed as an Alt-chord (`ESC b`
/// arriving together reads as Alt-b). `<A-x>` tokens emit `ESC x` within one
/// chunk on purpose — those *are* Alt-chords.
pub fn keys_to_chunks(spec: &str) -> Vec<Vec<u8>> {
    let mut chunks = vec![Vec::new()];
    let mut chars = spec.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '<' {
            let mut buf = [0u8; 4];
            chunks
                .last_mut()
                .unwrap()
                .extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        let mut token = String::new();
        let mut closed = false;
        for t in chars.by_ref() {
            if t == '>' {
                closed = true;
                break;
            }
            token.push(t);
        }
        assert!(closed, "unclosed key token <{token} in {spec:?}");
        chunks
            .last_mut()
            .unwrap()
            .extend_from_slice(&token_to_bytes(&token));
        if token == "Esc" {
            chunks.push(Vec::new());
        }
    }
    // A trailing empty chunk (spec ends in <Esc>) is kept on purpose: the
    // sender pauses before each chunk after the first, so the empty chunk
    // produces the required pause after a final Esc too.
    chunks
}

fn token_to_bytes(token: &str) -> Vec<u8> {
    match token {
        "Enter" | "CR" => b"\r".to_vec(),
        "Tab" => b"\t".to_vec(),
        "Esc" => b"\x1b".to_vec(),
        "BS" | "Backspace" => b"\x7f".to_vec(),
        "Space" => b" ".to_vec(),
        "lt" => b"<".to_vec(),
        "Up" => b"\x1b[A".to_vec(),
        "Down" => b"\x1b[B".to_vec(),
        "Right" => b"\x1b[C".to_vec(),
        "Left" => b"\x1b[D".to_vec(),
        "Home" => b"\x1b[H".to_vec(),
        "End" => b"\x1b[F".to_vec(),
        "Del" | "Delete" => b"\x1b[3~".to_vec(),
        "PageUp" => b"\x1b[5~".to_vec(),
        "PageDown" => b"\x1b[6~".to_vec(),
        "S-Tab" | "BackTab" => b"\x1b[Z".to_vec(),
        "C-Up" => b"\x1b[1;5A".to_vec(),
        "C-Down" => b"\x1b[1;5B".to_vec(),
        "C-Right" => b"\x1b[1;5C".to_vec(),
        "C-Left" => b"\x1b[1;5D".to_vec(),
        "S-Up" => b"\x1b[1;2A".to_vec(),
        "S-Down" => b"\x1b[1;2B".to_vec(),
        "S-Right" => b"\x1b[1;2C".to_vec(),
        "S-Left" => b"\x1b[1;2D".to_vec(),
        "S-Home" => b"\x1b[1;2H".to_vec(),
        "S-End" => b"\x1b[1;2F".to_vec(),
        "C-S-Right" => b"\x1b[1;6C".to_vec(),
        "C-S-Left" => b"\x1b[1;6D".to_vec(),
        "F1" => b"\x1bOP".to_vec(),
        "F2" => b"\x1bOQ".to_vec(),
        "F3" => b"\x1bOR".to_vec(),
        "F4" => b"\x1bOS".to_vec(),
        "F5" => b"\x1b[15~".to_vec(),
        "F6" => b"\x1b[17~".to_vec(),
        "F7" => b"\x1b[18~".to_vec(),
        "F8" => b"\x1b[19~".to_vec(),
        "F9" => b"\x1b[20~".to_vec(),
        "F10" => b"\x1b[21~".to_vec(),
        "F11" => b"\x1b[23~".to_vec(),
        "F12" => b"\x1b[24~".to_vec(),
        "A-Enter" => b"\x1b\r".to_vec(),
        _ => {
            if let Some(key) = token.strip_prefix("C-") {
                let [c] = key.as_bytes() else {
                    panic!("unsupported key token <{token}>");
                };
                assert!(
                    c.is_ascii_alphabetic() || *c == b' ',
                    "unsupported key token <{token}>"
                );
                return vec![c.to_ascii_uppercase() & 0x1f];
            }
            if let Some(key) = token
                .strip_prefix("A-")
                .or_else(|| token.strip_prefix("M-"))
            {
                let mut bytes = vec![0x1b];
                bytes.extend_from_slice(key.as_bytes());
                return bytes;
            }
            panic!("unsupported key token <{token}>");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(spec: &str) -> Vec<u8> {
        keys_to_chunks(spec).concat()
    }

    #[test]
    fn literals_and_tokens_mix() {
        assert_eq!(bytes("ab<Left>c"), b"ab\x1b[Dc");
        assert_eq!(bytes("<C-w>"), b"\x17");
        assert_eq!(bytes("<A-b>"), b"\x1bb");
        assert_eq!(bytes("<S-Left>"), b"\x1b[1;2D");
        assert_eq!(bytes("<F5>"), b"\x1b[15~");
        assert_eq!(bytes("<lt>x"), b"<x");
    }

    #[test]
    fn bare_esc_splits_chunks_but_alt_chords_do_not() {
        let chunks = keys_to_chunks("a<Esc>b");
        assert_eq!(chunks, vec![b"a\x1b".to_vec(), b"b".to_vec()]);
        // Trailing Esc keeps an empty chunk so the sender still pauses.
        let chunks = keys_to_chunks("a<Esc>");
        assert_eq!(chunks, vec![b"a\x1b".to_vec(), b"".to_vec()]);
        // Alt-chords stay in one chunk: they must arrive together.
        let chunks = keys_to_chunks("<A-Enter>x");
        assert_eq!(chunks, vec![b"\x1b\rx".to_vec()]);
    }

    #[test]
    #[should_panic(expected = "unclosed key token")]
    fn unclosed_token_panics() {
        keys_to_chunks("abc<Left");
    }

    #[test]
    #[should_panic(expected = "unsupported key token")]
    fn unknown_token_panics() {
        keys_to_chunks("<NoSuchKey>");
    }
}
