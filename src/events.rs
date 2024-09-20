use std::sync::{Mutex, Arc, Condvar};
use std::net::{SocketAddr};
use std::collections::VecDeque;
use crate::http::HTTPHeader;

pub enum Event {
    Connection {
        client_addr: SocketAddr,
    },
    HTTPRequest {
        path: String,
        headers: Vec<HTTPHeader>,
        body: Vec<u8>,
    },
}

#[derive(Clone)]
pub struct EventQueue {
    events: Arc<Mutex<VecDeque<Event>>>,
    cvar: Arc<Condvar>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(VecDeque::new())),
            cvar: Arc::new(Condvar::new()),
        }
    }
    pub fn dispatch(&mut self, event: Event) -> Result<(), &'static str> {
        self.events.lock().expect("Could not retrieve lock on events queue").push_back(event);
        self.cvar.notify_all();
        Ok(())
    }
    // blocks until an event is dispatched
    pub fn retrieve(&self) -> Result<Event, &'static str> {
        let mut events = self.events.lock().expect("Could not retrieve lock on events queue");
        events = self.cvar.wait(events).expect("Error waiting for condvar");
        let ev = events.pop_front();
        if let Some(ev) = ev {
            Ok(ev)
        } else {
            Err("No such event")
        }
    }
}
