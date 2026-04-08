use std::collections::VecDeque;

use crate::filter_eval::DriverEvent;

#[derive(Debug, Clone)]
pub struct EventQueue {
    capacity: usize,
    entries: VecDeque<DriverEvent>,
}

impl EventQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: VecDeque::new(),
        }
    }

    pub fn push(&mut self, event: DriverEvent) {
        if self.capacity == 0 {
            return;
        }
        if self.entries.len() == self.capacity {
            let _ = self.entries.pop_front();
        }
        self.entries.push_back(event);
    }

    pub fn pop(&mut self) -> Option<DriverEvent> {
        self.entries.pop_front()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
