use crate::data::source::SourceId;

/// Current playback mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackMode {
    Stopped,
    Playing,
    Paused,
}

/// How to interpret and display the time column values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeUnit {
    /// Values are epoch seconds (or similar) — display as HH:MM:SS with "s" suffix.
    Seconds,
    /// Unit is unknown — display raw values without a unit suffix.
    Raw,
}

/// Playback engine state — controls temporal replay of static data.
#[derive(Debug, Clone)]
pub struct PlaybackState {
    /// Current transport mode.
    pub mode: PlaybackMode,
    /// Which source is being played back.
    pub source_id: Option<SourceId>,
    /// Column name containing timestamp values.
    pub time_column: Option<String>,
    /// Current playback timestamp (in data units — typically epoch seconds).
    pub current_time: f64,
    /// Data time range: (min, max).
    pub time_range: (f64, f64),
    /// Speed multiplier: 1.0 = real-time, 10.0 = 10x faster.
    pub speed: f64,
    /// Trail duration in data units. None = show all data up to cursor.
    pub trail_duration: Option<f64>,
    /// Whether to loop when reaching the end.
    pub loop_enabled: bool,
    /// How to interpret the time column values for display.
    pub time_unit: TimeUnit,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            mode: PlaybackMode::Stopped,
            source_id: None,
            time_column: None,
            current_time: 0.0,
            time_range: (0.0, 0.0),
            speed: 1.0,
            trail_duration: None,
            loop_enabled: false,
            time_unit: TimeUnit::Raw,
        }
    }
}

impl PlaybackState {
    /// Whether playback is actively advancing time.
    pub fn is_playing(&self) -> bool {
        self.mode == PlaybackMode::Playing
    }

    /// Whether playback is configured and active (playing or paused).
    pub fn is_active(&self) -> bool {
        self.mode != PlaybackMode::Stopped
    }

    /// Total duration of the data in seconds.
    pub fn duration(&self) -> f64 {
        (self.time_range.1 - self.time_range.0).max(0.0)
    }

    /// Current progress as a fraction [0.0, 1.0].
    pub fn progress(&self) -> f32 {
        let d = self.duration();
        if d <= 0.0 {
            return 0.0;
        }
        ((self.current_time - self.time_range.0) / d).clamp(0.0, 1.0) as f32
    }

    /// Advance playback by real-world `dt` seconds (scaled by speed).
    /// Returns true if the temporal filter needs updating.
    pub fn advance(&mut self, dt: f64) -> bool {
        if self.mode != PlaybackMode::Playing {
            return false;
        }
        let prev = self.current_time;
        self.current_time += dt * self.speed;

        if self.current_time >= self.time_range.1 {
            if self.loop_enabled {
                self.current_time = self.time_range.0;
            } else {
                self.current_time = self.time_range.1;
                self.mode = PlaybackMode::Paused;
            }
        }

        (self.current_time - prev).abs() > 1e-9
    }

    /// Seek to a specific time value.
    pub fn seek(&mut self, time: f64) {
        self.current_time = time.clamp(self.time_range.0, self.time_range.1);
    }

    /// Step forward by one interval (1/100th of total duration, minimum 1 second).
    pub fn step_forward(&mut self) {
        let step = (self.duration() / 100.0).max(1.0);
        self.current_time = (self.current_time + step).min(self.time_range.1);
    }

    /// Step backward by one interval.
    pub fn step_backward(&mut self) {
        let step = (self.duration() / 100.0).max(1.0);
        self.current_time = (self.current_time - step).max(self.time_range.0);
    }

    /// Jump to the start of the data.
    pub fn jump_to_start(&mut self) {
        self.current_time = self.time_range.0;
    }

    /// Jump to the end of the data.
    pub fn jump_to_end(&mut self) {
        self.current_time = self.time_range.1;
        if self.mode == PlaybackMode::Playing {
            self.mode = PlaybackMode::Paused;
        }
    }

    /// Toggle between playing and paused.
    pub fn toggle_play_pause(&mut self) {
        match self.mode {
            PlaybackMode::Playing => self.mode = PlaybackMode::Paused,
            PlaybackMode::Paused => self.mode = PlaybackMode::Playing,
            PlaybackMode::Stopped => {}
        }
    }

    /// Stop playback and reset to start.
    pub fn stop(&mut self) {
        self.mode = PlaybackMode::Stopped;
        self.current_time = self.time_range.0;
    }

    /// Initialize playback for a source.
    pub fn init_for_source(
        &mut self,
        source_id: SourceId,
        time_column: String,
        time_min: f64,
        time_max: f64,
        time_unit: TimeUnit,
    ) {
        self.source_id = Some(source_id);
        self.time_column = Some(time_column);
        self.time_range = (time_min, time_max);
        self.current_time = time_min;
        self.mode = PlaybackMode::Paused;
        self.time_unit = time_unit;
    }

    /// Update the time column (and re-scan range) without stopping playback.
    pub fn set_time_column(
        &mut self,
        time_column: String,
        time_min: f64,
        time_max: f64,
        time_unit: TimeUnit,
    ) {
        self.time_column = Some(time_column);
        self.time_range = (time_min, time_max);
        self.current_time = time_min;
        self.time_unit = time_unit;
        // Pause when switching columns so user can review
        if self.mode == PlaybackMode::Playing {
            self.mode = PlaybackMode::Paused;
        }
    }
}
