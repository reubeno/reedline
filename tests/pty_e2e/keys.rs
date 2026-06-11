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
        for t in chars.by_ref() {
            if t == '>' {
                break;
            }
            token.push(t);
        }
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
