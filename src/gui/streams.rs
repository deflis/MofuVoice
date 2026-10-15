use serde::Serialize;
use std::collections::HashMap;
use tauri::ipc::Channel;

struct Connection<T> {
    id: String,
    channel: Channel<T>,
}

pub(super) struct WindowStreams<T> {
    connections: HashMap<String, Connection<T>>,
}
impl<T> Default for WindowStreams<T> {
    fn default() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }
}
impl<T: Serialize> WindowStreams<T> {
    pub fn connect(
        &mut self,
        label: &str,
        id: String,
        channel: Channel<T>,
        initial: T,
    ) -> Result<(), String> {
        channel.send(initial).map_err(|error| error.to_string())?;
        self.connections
            .insert(label.into(), Connection { id, channel });
        Ok(())
    }
    pub fn contains(&self, label: &str) -> bool {
        self.connections.contains_key(label)
    }
    pub fn send(&mut self, label: &str, message: T) -> Result<(), String> {
        if let Some(connection) = self.connections.get(label) {
            if let Err(error) = connection.channel.send(message) {
                self.connections.remove(label);
                return Err(error.to_string());
            }
        }
        Ok(())
    }
    pub fn disconnect(&mut self, label: &str, id: &str) -> bool {
        if self
            .connections
            .get(label)
            .is_some_and(|connection| connection.id == id)
        {
            self.connections.remove(label);
            true
        } else {
            false
        }
    }
    pub fn remove(&mut self, label: &str) {
        self.connections.remove(label);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn recording_channel() -> (Channel<u32>, Arc<Mutex<Vec<String>>>) {
        let messages = Arc::new(Mutex::new(Vec::new()));
        let received = messages.clone();
        let channel = Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(json) = body {
                received.lock().unwrap().push(json);
            }
            Ok(())
        });
        (channel, messages)
    }

    #[test]
    fn initial_state_precedes_updates_on_the_same_channel() {
        let mut streams = WindowStreams::default();
        let (channel, received) = recording_channel();
        streams.connect("main", "first".into(), channel, 1).unwrap();
        streams.send("main", 2).unwrap();
        assert_eq!(*received.lock().unwrap(), ["1", "2"]);
    }

    #[test]
    fn stale_disconnect_cannot_remove_a_replacement_connection() {
        let mut streams = WindowStreams::default();
        let (old, previous) = recording_channel();
        let (new, current) = recording_channel();
        streams.connect("main", "old".into(), old, 1).unwrap();
        streams.connect("main", "new".into(), new, 2).unwrap();
        assert!(!streams.disconnect("main", "old"));
        streams.send("main", 3).unwrap();
        assert_eq!(*previous.lock().unwrap(), ["1"]);
        assert_eq!(*current.lock().unwrap(), ["2", "3"]);
        assert!(streams.disconnect("main", "new"));
        assert!(!streams.disconnect("main", "new"));
    }

    #[test]
    fn failed_registration_preserves_an_existing_connection() {
        let mut streams = WindowStreams::default();
        let (old, received) = recording_channel();
        streams.connect("main", "old".into(), old, 1).unwrap();
        let failing = Channel::new(|_| Err(tauri::Error::AssetNotFound("test".into())));
        assert!(streams.connect("main", "new".into(), failing, 2).is_err());
        streams.send("main", 3).unwrap();
        assert_eq!(*received.lock().unwrap(), ["1", "3"]);
    }

    #[test]
    fn channels_are_isolated_and_failed_delivery_removes_the_subscriber() {
        let mut streams = WindowStreams::default();
        let sends = Arc::new(Mutex::new(0));
        let counter = sends.clone();
        let failing = Channel::new(move |_| {
            let mut count = counter.lock().unwrap();
            *count += 1;
            if *count > 1 {
                Err(tauri::Error::AssetNotFound("test".into()))
            } else {
                Ok(())
            }
        });
        streams.connect("main", "main".into(), failing, 1).unwrap();
        let (channel, received) = recording_channel();
        streams.connect("log", "log".into(), channel, 10).unwrap();
        assert!(streams.send("main", 2).is_err());
        assert!(!streams.contains("main"));
        streams.send("log", 11).unwrap();
        assert_eq!(*received.lock().unwrap(), ["10", "11"]);
    }
}
