use crate::data::source::{DataSource, SourceId};
use crossbeam_channel::{unbounded, Receiver, Sender};

/// Events sent from background threads to the UI thread.
pub enum DataEvent {
    /// A source finished loading successfully.
    Loaded(DataSource),
    /// A load failed — show an error in the UI.
    LoadError { id: SourceId, message: String },
}

/// Central application state — owned by DataVisualizerApp, read by all UI modules.
pub struct AppState {
    /// All currently loaded data sources, in load order.
    pub sources: Vec<DataSource>,

    /// Monotonically increasing ID counter for new sources.
    next_id: SourceId,

    /// Sender cloned and handed to background loader threads.
    pub event_tx: Sender<DataEvent>,
    /// Receiver polled each frame in update().
    pub event_rx: Receiver<DataEvent>,

    /// Non-fatal messages shown in the UI (e.g. load errors).
    pub notifications: Vec<String>,
}

impl Default for AppState {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            sources: Vec::new(),
            next_id: 0,
            event_tx: tx,
            event_rx: rx,
            notifications: Vec::new(),
        }
    }
}

impl AppState {
    /// Allocate and return the next source ID.
    pub fn next_source_id(&mut self) -> SourceId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Drain all pending events from background threads.
    /// Call once per frame from update().
    pub fn poll_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                DataEvent::Loaded(source) => {
                    self.sources.push(source);
                }
                DataEvent::LoadError { id, message } => {
                    self.notifications.push(format!("Source {}: {}", id, message));
                }
            }
        }
    }

    /// Remove a source by ID.
    pub fn remove_source(&mut self, id: SourceId) {
        self.sources.retain(|s| s.id != id);
    }

    pub fn has_sources(&self) -> bool {
        !self.sources.is_empty()
    }
}
