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

    /// Push a socket path, returning the assigned numeric ID. If the path is
    /// already present, its existing entry is dropped first so the fresh
    /// registration moves it to the top instead of duplicating it.
    pub fn push(&mut self, path: PathBuf) -> u32 {
        if let Some(pos) = self.entries.iter().position(|e| e.path == path) {
            self.entries.remove(pos);
        }
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(StackEntry { id, path });
        id
    }

    /// Remove the entry matching `path`, if any.
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
    fn push_existing_path_moves_to_top() {
        let mut s = AgentStack::new();
        s.push("/a".into());
        s.push("/b".into());
        let id = s.push("/a".into());
        assert_eq!(s.entries.len(), 2);
        assert_eq!(s.entries[0].path, PathBuf::from("/b"));
        assert_eq!(s.entries[1].path, PathBuf::from("/a"));
        assert_eq!(s.entries[1].id, id);
    }

    #[test]
    fn remove_existing_path() {
        let mut s = AgentStack::new();
        s.push("/a".into());
        let id_b = s.push("/b".into());
        let removed = s.remove(Path::new("/b")).unwrap();
        assert_eq!(removed, id_b);
        assert_eq!(s.entries.len(), 1);
        assert_eq!(s.entries[0].path, PathBuf::from("/a"));
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
