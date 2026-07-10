use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AgentStack {
    entries: Vec<PathBuf>,
}

pub type SharedStack = Arc<Mutex<AgentStack>>;

impl AgentStack {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn shared() -> SharedStack {
        Arc::new(Mutex::new(Self::new()))
    }

    /// Push a socket path onto the top of the stack (back of the vec).
    pub fn push(&mut self, path: PathBuf) {
        tracing::info!(path = %path.display(), "registered socket");
        self.entries.push(path);
    }

    /// Remove the most-recently-registered occurrence of `path`.
    pub fn remove(&mut self, path: &Path) {
        if let Some(pos) = self.entries.iter().rposition(|p| p == path) {
            self.entries.remove(pos);
            tracing::info!(path = %path.display(), "deregistered socket");
        }
    }

    /// Walk from top (back) downward. Remove entries whose socket file no
    /// longer exists. Return the first path whose file still exists, or
    /// `None` if the stack is empty after pruning.
    ///
    /// This does NOT attempt a socket connection — callers do that
    /// asynchronously and call `remove` if the connect fails.
    pub fn drain_until_live(&mut self) -> Option<PathBuf> {
        while let Some(top) = self.entries.last() {
            if top.exists() {
                return Some(top.clone());
            }
            let dead = self.entries.pop().unwrap();
            tracing::info!(path = %dead.display(), "pruned missing socket");
        }
        None
    }

    pub fn snapshot(&self) -> Vec<PathBuf> {
        self.entries.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_lifo_order() {
        let mut s = AgentStack::new();
        s.entries.push("/a".into());
        s.entries.push("/b".into());
        s.entries.push("/c".into());
        assert_eq!(s.entries.last().unwrap(), Path::new("/c"));
    }

    #[test]
    fn remove_last_occurrence() {
        let mut s = AgentStack::new();
        s.entries.push("/a".into());
        s.entries.push("/b".into());
        s.entries.push("/a".into());
        s.remove(Path::new("/a"));
        assert_eq!(s.entries, vec![PathBuf::from("/a"), PathBuf::from("/b")]);
    }

    #[test]
    fn drain_until_live_skips_missing() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let live = tmp.path().to_path_buf();
        let mut s = AgentStack::new();
        // live was registered first (oldest); the two nonexistent entries are newer (on top).
        s.entries.push(live.clone());
        s.entries.push("/nonexistent-1".into());
        s.entries.push("/nonexistent-2".into());
        let result = s.drain_until_live();
        assert_eq!(result, Some(live));
        assert_eq!(s.entries.len(), 1);
    }
}
