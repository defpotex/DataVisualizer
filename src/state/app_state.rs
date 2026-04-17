use crate::data::filter::Filter;
use crate::data::source::{DataSource, SourceId};
use crate::data::udp_receiver::UdpReceiverHandle;
use crate::plot::plot_config::PlotConfig;
use crate::plot::sync::PlotSyncEvent;
use crate::state::perf_settings::PerformanceSettings;
use crate::state::playback::PlaybackState;
use crate::state::selection::SelectionSet;
use crossbeam_channel::{unbounded, Receiver, Sender};

/// Events sent from background threads to the UI thread.
pub enum DataEvent {
    /// A source finished loading successfully.
    Loaded(DataSource),
    /// A load failed — show an error in the UI.
    LoadError { id: SourceId, message: String },
    /// A plot's background data sync completed.
    PlotSyncReady(PlotSyncEvent),
    /// A streaming source has updated data — replaces the existing source with the same ID.
    StreamUpdate(DataSource),
}

/// Central application state — owned by DataVisualizerApp, read by all UI modules.
pub struct AppState {
    /// All currently loaded data sources, in load order.
    pub sources: Vec<DataSource>,

    /// Monotonically increasing ID counter for new sources.
    next_source_id: SourceId,

    /// Serializable plot configurations (used for session persistence).
    pub plots: Vec<PlotConfig>,

    /// Monotonically increasing ID counter for new plots.
    plot_id_counter: usize,

    /// Active attribute filters applied to all plots.
    pub filters: Vec<Filter>,

    /// Monotonically increasing ID counter for new filters.
    filter_id_counter: usize,

    /// Sender cloned and handed to background loader threads.
    pub event_tx: Sender<DataEvent>,
    /// Receiver polled each frame in update().
    pub event_rx: Receiver<DataEvent>,

    /// Non-fatal messages shown in the UI (e.g. load errors).
    pub notifications: Vec<String>,

    /// User-tunable performance settings.
    pub perf: PerformanceSettings,

    /// Currently active point selection (global — one at a time).
    pub selection: Option<SelectionSet>,

    /// Playback engine state for temporal replay.
    pub playback: PlaybackState,

    /// Active UDP stream receiver handles, keyed by source ID.
    pub udp_handles: Vec<UdpReceiverHandle>,
    /// Source IDs that are streaming (for UI display).
    pub streaming_source_ids: Vec<SourceId>,
}

impl Default for AppState {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            sources: Vec::new(),
            next_source_id: 0,
            plots: Vec::new(),
            plot_id_counter: 0,
            event_tx: tx,
            event_rx: rx,
            notifications: Vec::new(),
            perf: PerformanceSettings::default(),
            filters: Vec::new(),
            filter_id_counter: 0,
            selection: None,
            playback: PlaybackState::default(),
            udp_handles: Vec::new(),
            streaming_source_ids: Vec::new(),
        }
    }
}

impl AppState {
    /// Allocate and return the next source ID.
    pub fn next_source_id(&mut self) -> SourceId {
        let id = self.next_source_id;
        self.next_source_id += 1;
        id
    }

    /// Allocate and return the next plot ID.
    pub fn alloc_plot_id(&mut self) -> usize {
        let id = self.plot_id_counter;
        self.plot_id_counter += 1;
        id
    }

    /// Drain all pending events from background threads.
    /// Returns (`had_events`, `sync_events`, `had_stream_update`).
    /// The caller should request repaint if `had_events` is true,
    /// route `sync_events` to `PlotManager`, and re-sync streaming plots
    /// if `had_stream_update` is true.
    pub fn poll_events(&mut self) -> (bool, Vec<PlotSyncEvent>, bool) {
        let mut got_event = false;
        let mut sync_events: Vec<PlotSyncEvent> = Vec::new();
        let mut had_stream_update = false;
        while let Ok(event) = self.event_rx.try_recv() {
            got_event = true;
            match event {
                DataEvent::Loaded(source) => {
                    self.sources.push(source);
                }
                DataEvent::LoadError { id, message } => {
                    self.notifications.push(format!("Source {}: {}", id, message));
                }
                DataEvent::StreamUpdate(source) => {
                    // Replace the existing source with same ID, or insert if new.
                    if let Some(existing) = self.sources.iter_mut().find(|s| s.id == source.id) {
                        *existing = source;
                    } else {
                        self.sources.push(source);
                    }
                    had_stream_update = true;
                }
                DataEvent::PlotSyncReady(sync_evt) => {
                    sync_events.push(sync_evt);
                }
            }
        }
        (got_event, sync_events, had_stream_update)
    }

    /// Remove a source by ID.
    pub fn remove_source(&mut self, id: SourceId) {
        self.sources.retain(|s| s.id != id);
    }

    /// Allocate and return the next filter ID.
    pub fn alloc_filter_id(&mut self) -> usize {
        let id = self.filter_id_counter;
        self.filter_id_counter += 1;
        id
    }

    /// Returns true if any enabled filter has changed since last call
    /// (currently unused — caller re-syncs on any filter mutation).
    pub fn has_active_filters(&self) -> bool {
        self.filters.iter().any(|f| f.enabled)
    }

    pub fn has_sources(&self) -> bool {
        !self.sources.is_empty()
    }

    /// Check if a source is a live UDP stream.
    pub fn is_streaming(&self, id: SourceId) -> bool {
        self.streaming_source_ids.contains(&id)
    }

    /// Stop and remove a UDP stream by source ID.
    pub fn stop_stream(&mut self, id: SourceId) {
        if let Some(pos) = self.udp_handles.iter().position(|h| h.source_id == id) {
            self.udp_handles[pos].stop();
            self.udp_handles.remove(pos);
        }
        self.streaming_source_ids.retain(|&sid| sid != id);
    }
}
