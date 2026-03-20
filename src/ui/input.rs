/// Keyboard input translation: Wayland keysym + modifiers → terminal bytes.
use smithay_client_toolkit::seat::keyboard::{Keysym, Modifiers};

/// Convert a key event into the byte sequence to send to the PTY.
/// Returns `None` if the key should be ignored.
pub fn key_to_bytes(keysym: Keysym, utf8: Option<&str>, modifiers: &Modifiers) -> Option<Vec<u8>> {
    // Special keys first (independent of modifiers)
    let special: Option<&[u8]> = match keysym {
        Keysym::Return | Keysym::KP_Enter => Some(b"\r"),
        Keysym::BackSpace               => Some(b"\x7f"),
        Keysym::Tab                     => Some(b"\t"),
        Keysym::Escape                  => Some(b"\x1b"),
        Keysym::Up                      => Some(b"\x1b[A"),
        Keysym::Down                    => Some(b"\x1b[B"),
        Keysym::Right                   => Some(b"\x1b[C"),
        Keysym::Left                    => Some(b"\x1b[D"),
        Keysym::Home                    => Some(b"\x1b[H"),
        Keysym::End                     => Some(b"\x1b[F"),
        Keysym::Delete                  => Some(b"\x1b[3~"),
        Keysym::Page_Up                 => Some(b"\x1b[5~"),
        Keysym::Page_Down               => Some(b"\x1b[6~"),
        // F1–F12
        Keysym::F1  => Some(b"\x1b[11~"),
        Keysym::F2  => Some(b"\x1b[12~"),
        Keysym::F3  => Some(b"\x1b[13~"),
        Keysym::F4  => Some(b"\x1b[14~"),
        Keysym::F5  => Some(b"\x1b[15~"),
        Keysym::F6  => Some(b"\x1b[17~"),
        Keysym::F7  => Some(b"\x1b[18~"),
        Keysym::F8  => Some(b"\x1b[19~"),
        Keysym::F9  => Some(b"\x1b[20~"),
        Keysym::F10 => Some(b"\x1b[21~"),
        Keysym::F11 => Some(b"\x1b[23~"),
        Keysym::F12 => Some(b"\x1b[24~"),
        _           => None,
    };

    if let Some(seq) = special {
        return Some(seq.to_vec());
    }

    // Ctrl + printable char → ctrl byte
    if modifiers.ctrl {
        if let Some(s) = utf8 {
            if let Some(ch) = s.chars().next() {
                let lower = ch.to_ascii_lowercase();
                if lower >= 'a' && lower <= 'z' {
                    let ctrl_byte = (lower as u8) - b'a' + 1;
                    return Some(vec![ctrl_byte]);
                }
                // Ctrl+@ = 0x00, Ctrl+[ = 0x1b, Ctrl+\ = 0x1c, etc.
                match ch {
                    '@' => return Some(vec![0x00]),
                    '[' => return Some(vec![0x1b]),
                    '\\' => return Some(vec![0x1c]),
                    ']' => return Some(vec![0x1d]),
                    '^' => return Some(vec![0x1e]),
                    '_' => return Some(vec![0x1f]),
                    _ => {}
                }
            }
        }
    }

    // Regular printable chars → UTF-8
    if let Some(s) = utf8 {
        if !s.is_empty() {
            return Some(s.as_bytes().to_vec());
        }
    }

    None
}
