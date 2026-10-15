use std::sync::Mutex;

/// Server-side clipboard. Text only for now; Apple's compressed pasteboard payloads
/// (client messages 11/31, server message 31) are not decoded yet.
pub trait Clipboard: Send + Sync {
    /// Text the viewer copied.
    fn set_from_client(&self, text: String);
    /// Current server clipboard with a serial that changes on every server-side update.
    fn server_text(&self) -> Option<(u64, String)>;
}

#[derive(Default)]
pub struct MemoryClipboard {
    state: Mutex<(u64, Option<String>)>,
}

impl MemoryClipboard {
    /// Simulates a copy on the server side.
    pub fn set_server_text(&self, text: impl Into<String>) {
        let mut s = self.state.lock().unwrap();
        s.0 += 1;
        s.1 = Some(text.into());
    }

    pub fn last(&self) -> Option<String> {
        self.state.lock().unwrap().1.clone()
    }
}

impl Clipboard for MemoryClipboard {
    fn set_from_client(&self, text: String) {
        let mut s = self.state.lock().unwrap();
        s.0 += 1;
        s.1 = Some(text);
    }

    fn server_text(&self) -> Option<(u64, String)> {
        let s = self.state.lock().unwrap();
        s.1.clone().map(|t| (s.0, t))
    }
}

/// Tracks what the viewer already has so changes are sent once and client copies are not echoed.
#[derive(Default)]
pub struct ClipboardSync {
    seen: u64,
}

impl ClipboardSync {
    pub fn from_client(&mut self, cb: &dyn Clipboard, text: String) {
        cb.set_from_client(text);
        if let Some((serial, _)) = cb.server_text() {
            self.seen = serial;
        }
    }

    pub fn pending(&mut self, cb: &dyn Clipboard) -> Option<String> {
        let (serial, text) = cb.server_text()?;
        (serial != self.seen).then(|| {
            self.seen = serial;
            text
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_sends_server_changes_once_and_does_not_echo() {
        let cb = MemoryClipboard::default();
        let mut sync = ClipboardSync::default();
        assert_eq!(sync.pending(&cb), None);
        cb.set_server_text("one");
        assert_eq!(sync.pending(&cb).as_deref(), Some("one"));
        assert_eq!(sync.pending(&cb), None);
        sync.from_client(&cb, "from viewer".into());
        assert_eq!(cb.last().as_deref(), Some("from viewer"));
        assert_eq!(sync.pending(&cb), None);
        cb.set_server_text("two");
        assert_eq!(sync.pending(&cb).as_deref(), Some("two"));
    }
}
