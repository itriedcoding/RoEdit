//! Undo/redo. The editor records a snapshot before each mutation. Snapshots are
//! cheap because meshes and images are stored behind `Arc` and therefore shared
//! with the live document until they are edited.

#[derive(Debug)]
pub struct History<T> {
    undo: Vec<(String, T)>,
    redo: Vec<(String, T)>,
    limit: usize,
    /// Index of the snapshot that matches the last saved state (for the
    /// "unsaved changes" indicator), counted from the oldest surviving entry.
    saved_depth: Option<usize>,
}

impl<T: Clone> Default for History<T> {
    fn default() -> Self {
        Self::new(100)
    }
}

impl<T: Clone> History<T> {
    pub fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: limit.max(1),
            saved_depth: None,
        }
    }

    /// Records the state *before* a mutation. Clears the redo stack.
    pub fn record(&mut self, label: impl Into<String>, before: T) {
        if self.undo.len() >= self.limit {
            self.undo.remove(0);
            self.saved_depth = self.saved_depth.and_then(|d| d.checked_sub(1));
        }
        self.undo.push((label.into(), before));
        self.redo.clear();
    }

    /// Restores the previous state, pushing `current` onto the redo stack.
    pub fn undo(&mut self, current: &T) -> Option<(String, T)> {
        let (label, state) = self.undo.pop()?;
        self.redo.push((label.clone(), current.clone()));
        Some((label, state))
    }

    pub fn redo(&mut self, current: &T) -> Option<(String, T)> {
        let (label, state) = self.redo.pop()?;
        self.undo.push((label.clone(), current.clone()));
        Some((label, state))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|(l, _)| l.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|(l, _)| l.as_str())
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.saved_depth = None;
    }

    pub fn mark_saved(&mut self) {
        self.saved_depth = Some(self.undo.len());
    }

    /// True when the document differs from the last saved state. Conservative:
    /// reports "modified" when in doubt.
    pub fn is_modified(&self) -> bool {
        match self.saved_depth {
            Some(d) => d != self.undo.len(),
            None => !self.undo.is_empty(),
        }
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit.max(1);
        while self.undo.len() > self.limit {
            self.undo.remove(0);
            self.saved_depth = self.saved_depth.and_then(|d| d.checked_sub(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_cycle() {
        let mut h: History<i32> = History::new(8);
        h.record("first", 0);
        h.record("second", 1);
        assert!(h.can_undo());
        assert_eq!(h.undo_label(), Some("second"));
        let (label, state) = h.undo(&2).unwrap();
        assert_eq!(label, "second");
        assert_eq!(state, 1);
        assert!(h.can_redo());
        let (_, state) = h.redo(&1).unwrap();
        assert_eq!(state, 2);
    }

    #[test]
    fn limit_drops_oldest() {
        let mut h: History<i32> = History::new(3);
        for i in 0..10 {
            h.record(format!("step {i}"), i);
        }
        assert_eq!(h.undo_len(), 3);
        assert_eq!(h.undo_label(), Some("step 9"));
    }

    #[test]
    fn new_action_clears_redo() {
        let mut h: History<i32> = History::new(8);
        h.record("a", 1);
        h.undo(&2);
        assert!(h.can_redo());
        h.record("b", 3);
        assert!(!h.can_redo());
    }

    #[test]
    fn modified_flag_tracks_save() {
        let mut h: History<i32> = History::new(4);
        assert!(!h.is_modified());
        h.record("a", 1);
        assert!(h.is_modified());
        h.mark_saved();
        assert!(!h.is_modified());
        h.record("b", 2);
        assert!(h.is_modified());
    }
}
