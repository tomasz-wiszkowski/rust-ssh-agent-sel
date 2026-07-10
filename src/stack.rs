use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct StackEntry {
    pub id: u32,
    pub path: PathBuf,
}

pub struct AgentStack {
    entries: Vec<StackEntry>,
    next_id: u32,
}

pub type SharedStack = Arc<Mutex<AgentStack>>;

impl AgentStack {
    pub fn new() -> Self {
        Self { entries: Vec::new(), next_id: 1 }
    }

    pub fn shared() -> SharedStack {
        Arc::new(Mutex::new(Self::new()))
    }

    /// Push a socket path. Returns the assigned numeric ID.
    pub fn push(&mut self, path: PathBuf) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(StackEntry { id, path });
        id
    }

    /// Remove the most-recently-registered occurrence of `path`.
    /// Returns the ID of the removed entry, or `None` if not found.
    pub fn remove(&mut self, path: &Path) -> Option<u32> {
        let pos = self.entries.iter().rposition(|e| e.path == path)?;
        Some(self.entries.remove(pos).id)
    }

    /// Walk from top (back) downward. Remove entries whose socket file no
    /// longer exists, logging each removal. Return the first entry whose
    /// file still exists, without removing it from the stack.
    pub fn drain_until_live(&mut self) -> Option<StackEntry> {
        while let Some(top) = self.entries.last() {
            if top.path.exists() {
                return Some(top.clone());
            }
            let dead = self.entries.pop().unwrap();
            tracing::info!("socket [{}] gone; removing", dead.id);
        }
        None
    }

    pub fn snapshot(&self) -> Vec<StackEntry> {
        self.entries.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_lifo_order() {
        let mut s = AgentStack::new();
        s.push("/a".into());
        s.push("/b".into());
        let id_c = s.push("/c".into());
        assert_eq!(s.entries.last().unwrap().id, id_c);
    }

    #[test]
    fn remove_last_occurrence() {
        let mut s = AgentStack::new();
        let id_a1 = s.push("/a".into());
        s.push("/b".into());
        s.push("/a".into());
        // rposition finds the last /a — which is NOT id_a1
        let removed = s.remove(Path::new("/a")).unwrap();
        assert_ne!(removed, id_a1);
        assert_eq!(s.entries.len(), 2);
        assert_eq!(s.entries[0].path, PathBuf::from("/a"));
        assert_eq!(s.entries[1].path, PathBuf::from("/b"));
    }

    #[test]
    fn drain_until_live_skips_missing() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let live = tmp.path().to_path_buf();
        let mut s = AgentStack::new();
        s.push(live.clone());
        s.push("/nonexistent-1".into());
        s.push("/nonexistent-2".into());
        let result = s.drain_until_live();
        assert_eq!(result.map(|e| e.path), Some(live));
        assert_eq!(s.entries.len(), 1);
    }
}
