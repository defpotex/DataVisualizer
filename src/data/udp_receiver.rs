use crate::data::schema::DataSchema;
use crate::data::source::{DataSource, SourceId};
use crate::state::app_state::DataEvent;
use crossbeam_channel::Sender;
use polars::prelude::*;
use polars::io::SerReader;
use std::io::Cursor;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ── UdpStreamConfig ──────────────────────────────────────────────────────────

/// User-specified configuration for a UDP stream source.
#[derive(Debug, Clone)]
pub struct UdpStreamConfig {
    pub bind_addr: String,
    pub max_rows: usize,
    pub label: String,
}

impl Default for UdpStreamConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:5005".to_string(),
            max_rows: 100_000,
            label: "UDP Stream".to_string(),
        }
    }
}

// ── UdpReceiverHandle ────────────────────────────────────────────────────────

/// Handle returned to the UI thread for controlling a running UDP receiver.
pub struct UdpReceiverHandle {
    /// Set to true to stop the receiver thread.
    pub stop: Arc<AtomicBool>,
    /// Set to true to pause ingestion (socket stays open, packets are discarded).
    pub paused: Arc<AtomicBool>,
    /// The source ID assigned to this stream.
    pub source_id: SourceId,
}

impl UdpReceiverHandle {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
    }

    pub fn toggle_pause(&self) {
        let was = self.paused.load(Ordering::SeqCst);
        self.paused.store(!was, Ordering::SeqCst);
    }
}

// ── start_udp_receiver ───────────────────────────────────────────────────────

/// Spawn a background thread that listens for UDP packets, parses them as CSV
/// rows, accumulates into a DataFrame, and periodically sends updated
/// `DataSource` snapshots to the UI thread.
///
/// Returns a handle for pause/stop control.
pub fn start_udp_receiver(
    source_id: SourceId,
    config: UdpStreamConfig,
    tx: Sender<DataEvent>,
) -> Result<UdpReceiverHandle, String> {
    let socket = UdpSocket::bind(&config.bind_addr)
        .map_err(|e| format!("Could not bind UDP socket to {}: {}", config.bind_addr, e))?;

    // Non-blocking with a short timeout so we can check stop/pause flags.
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| format!("Could not set socket timeout: {}", e))?;

    let stop = Arc::new(AtomicBool::new(false));
    let paused = Arc::new(AtomicBool::new(false));

    let handle = UdpReceiverHandle {
        stop: Arc::clone(&stop),
        paused: Arc::clone(&paused),
        source_id,
    };

    let label = config.label.clone();
    let max_rows = config.max_rows;

    std::thread::spawn(move || {
        receiver_loop(source_id, label, max_rows, socket, stop, paused, tx);
    });

    Ok(handle)
}

// ── Receiver loop ────────────────────────────────────────────────────────────

fn receiver_loop(
    source_id: SourceId,
    label: String,
    max_rows: usize,
    socket: UdpSocket,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    tx: Sender<DataEvent>,
) {
    let mut buf = [0u8; 65536];
    let mut header: Option<Vec<String>> = None;
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut dirty = false;
    let mut last_flush = Instant::now();

    // Flush interval: send updated DataSource to UI at most this often.
    let flush_interval = Duration::from_millis(250);

    while !stop.load(Ordering::SeqCst) {
        // Receive a packet.
        let n = match socket.recv_from(&mut buf) {
            Ok((n, _addr)) => n,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut => {
                // No data available — check if we should flush.
                if dirty && last_flush.elapsed() >= flush_interval {
                    flush_to_ui(source_id, &label, &header, &rows, &tx);
                    dirty = false;
                    last_flush = Instant::now();
                }
                continue;
            }
            Err(_) => {
                // Socket error — stop.
                break;
            }
        };

        if paused.load(Ordering::SeqCst) {
            continue;
        }

        let text = match std::str::from_utf8(&buf[..n]) {
            Ok(s) => s.trim().to_string(),
            Err(_) => continue,
        };

        if text.is_empty() {
            continue;
        }

        // Handle multiple lines in one packet (shouldn't happen with our streamer,
        // but be robust).
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Check for explicit header marker from udp_streamer: "#HEADER:col1,col2,..."
            if let Some(hdr_str) = line.strip_prefix("#HEADER:") {
                let new_header: Vec<String> = hdr_str.split(',').map(|s| s.trim().to_string()).collect();
                if header.is_none() || header.as_ref().map(|h| h.len()) != Some(new_header.len()) {
                    // First header or schema change — reset accumulated rows.
                    rows.clear();
                    dirty = true;
                }
                header = Some(new_header);
                continue;
            }

            let fields: Vec<String> = line.split(',').map(|s| s.trim().to_string()).collect();

            if header.is_none() {
                // No explicit header received yet — generate generic column names.
                // We don't try to auto-detect headers by content because datasets
                // with text columns (e.g. callsign) make every row look like a header.
                header = Some(
                    (0..fields.len())
                        .map(|i| format!("col_{}", i))
                        .collect(),
                );
                // Fall through to add this as a data row.
            }

            // Pad or truncate to match header width.
            let hdr = header.as_ref().unwrap();
            let mut row = fields;
            row.resize(hdr.len(), String::new());
            row.truncate(hdr.len());

            rows.push(row);
            dirty = true;

            // Rolling buffer: drop oldest rows if over max.
            if rows.len() > max_rows {
                let excess = rows.len() - max_rows;
                rows.drain(..excess);
            }
        }

        // Periodic flush to UI.
        if dirty && last_flush.elapsed() >= flush_interval {
            flush_to_ui(source_id, &label, &header, &rows, &tx);
            dirty = false;
            last_flush = Instant::now();
        }
    }

    // Final flush.
    if dirty {
        flush_to_ui(source_id, &label, &header, &rows, &tx);
    }
}

/// Build a polars DataFrame from accumulated rows and send to UI.
fn flush_to_ui(
    source_id: SourceId,
    label: &str,
    header: &Option<Vec<String>>,
    rows: &[Vec<String>],
    tx: &Sender<DataEvent>,
) {
    let hdr = match header {
        Some(h) => h,
        None => return,
    };
    if rows.is_empty() {
        return;
    }

    // Build CSV string and parse with polars for automatic type inference.
    let mut csv_buf = String::with_capacity(rows.len() * 80);
    csv_buf.push_str(&hdr.join(","));
    csv_buf.push('\n');
    for row in rows {
        csv_buf.push_str(&row.join(","));
        csv_buf.push('\n');
    }

    let cursor = Cursor::new(csv_buf.as_bytes());
    let df = match CsvReader::new(cursor).finish() {
        Ok(df) => df,
        Err(_) => return,
    };

    let schema = DataSchema::infer(&df);
    let source = DataSource {
        id: source_id,
        label: label.to_string(),
        path: None,
        schema,
        df,
        column_aliases: std::collections::HashMap::new(),
    };

    // Use StreamUpdate event — UI will replace the existing source with same ID.
    let _ = tx.send(DataEvent::StreamUpdate(source));
}
