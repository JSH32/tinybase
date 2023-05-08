use std::sync::mpsc::Receiver;

use crate::{table::SenderMap, Record};

#[derive(Clone)]
pub(crate) enum Event<T> {
    Remove(Record<T>),
    Insert(Record<T>),
    Update { id: u64, old_data: T, new_data: T },
}

pub(crate) struct Subscriber<T> {
    id: u64,
    pub rx: Receiver<Event<T>>,
    senders: SenderMap<Event<T>>,
}

impl<T> Subscriber<T> {
    pub fn new(id: u64, rx: Receiver<Event<T>>, senders: SenderMap<Event<T>>) -> Self {
        Self { id, rx, senders }
    }
}

impl<T> Drop for Subscriber<T> {
    fn drop(&mut self) {
        self.senders.write().unwrap().remove(&self.id);
    }
}
