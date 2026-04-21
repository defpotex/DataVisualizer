use polars::prelude::*;
use std::net::UdpSocket;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ── Config ────────────────────────────────────────────────────────────────────

struct Config {
    file: PathBuf,
    target: String,
    speed: f64,
    loop_mode: bool,
    send_header: bool,
}

fn parse_args() -> Result<Config, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut file: Option<PathBuf> = None;
    let mut target = "127.0.0.1:5005".to_string();
    let mut speed = 1.0f64;
    let mut loop_mode = false;
    let mut send_header = true;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => { print_usage(); std::process::exit(0); }
            "--file" => {
                i += 1;
                file = Some(PathBuf::from(args.get(i).ok_or("--file needs a value")?));
            }
            "--target" => {
                i += 1;
                target = args.get(i).ok_or("--target needs a value")?.clone();
            }
            "--speed" => {
                i += 1;
                speed = args.get(i).ok_or("--speed needs a value")?
                    .parse::<f64>().map_err(|_| "--speed must be a number")?;
                if speed <= 0.0 {
                    return Err("--speed must be > 0".to_string());
                }
            }
            "--loop"   => loop_mode = true,
            "--header" => send_header = true, // kept for backwards compat (now default)
            "--no-header" => send_header = false,
            other => return Err(format!("Unknown argument: {}", other)),
        }
        i += 1;
    }

    Ok(Config {
        file: file.ok_or("--file <PATH> is required")?,
        target,
        speed,
        loop_mode,
        send_header,
    })
}

fn print_usage() {
    println!(
        r#"udp_streamer — replay a CSV file over UDP

USAGE:
  cargo run -p udp_streamer -- --file <PATH> [OPTIONS]

OPTIONS:
  --file <PATH>        CSV file to replay [required]
  --target <HOST:PORT> UDP destination    [default: 127.0.0.1:5005]
  --speed <MULT>       Playback speed multiplier [default: 1.0]
                         1.0 = real-time  |  60.0 = 1 min/sec  |  0 = max rate
  --loop               Restart from beginning when file ends
  --no-header          Suppress CSV header packets (header sent by default)
  --help               Print this message

EXAMPLES:
  cargo run -p udp_streamer -- --file test_data/adsb_conus.csv --speed 50
  cargo run -p udp_streamer -- --file test_data/adsb_conus.csv --speed 10 --loop
  cargo run -p udp_streamer -- --file test_data/adsb_conus.csv --target 192.168.1.10:9000

VERIFY (no main app needed):
  # Terminal 1 — listen for packets
  nc -ul 5005

  # Terminal 2 — stream at 100x speed
  cargo run -p udp_streamer -- --file test_data/adsb_conus.csv --speed 100
"#
    );
}

// ── CSV loading ───────────────────────────────────────────────────────────────

struct CsvData {
    /// Header row as comma-separated string
    header: String,
    /// Each row as a pre-formatted "f1,f2,...\n" string, sorted by timestamp
    rows: Vec<String>,
    /// Timestamp values parallel to rows (seconds, for delay calculation)
    timestamps: Vec<i64>,
}

fn load_csv(path: &PathBuf) -> Result<CsvData, String> {
    println!("  Loading {}…", path.display());

    let df = LazyCsvReader::new(path)
        .with_has_header(true)
        .finish()
        .map_err(|e| format!("Could not read CSV: {}", e))?
        .sort(
            ["timestamp"],
            SortMultipleOptions::default().with_order_descending(false),
        )
        .collect()
        .map_err(|e| format!("Could not sort CSV: {}", e))?;

    let n = df.height();
    if n == 0 {
        return Err("CSV file is empty".to_string());
    }

    // Build header string
    let header = df.get_column_names()
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(",");

    // Extract timestamp column for timing
    let ts_col = df.column("timestamp")
        .map_err(|_| "CSV has no 'timestamp' column — required for timed replay")?
        .clone();

    // Cast to Int64 if needed (CSV reader may produce Utf8 or Float64).
    // Column::cast returns Column; call .as_series() to get a &Series.
    let ts_col_i64 = ts_col
        .cast(&DataType::Int64)
        .map_err(|e| format!("Could not cast timestamp column to Int64: {}", e))?;
    let ts_ca = ts_col_i64
        .as_series()
        .expect("Column should have a Series after cast")
        .i64()
        .map_err(|e| format!("Could not read timestamp column as integer: {}", e))?;

    let timestamps: Vec<i64> = ts_ca
        .into_iter()
        .map(|v: Option<i64>| v.unwrap_or(0))
        .collect();

    // Pre-format every row as "v1,v2,...\n"
    println!("  Formatting {} rows…", n);
    let mut rows: Vec<String> = Vec::with_capacity(n);
    for i in 0..n {
        let row = df.get_row(i).map_err(|e| format!("Row read error: {}", e))?;
        let formatted = row.0.iter()
            .map(|v| match v {
                AnyValue::Null       => String::new(),
                AnyValue::Boolean(b) => b.to_string(),
                AnyValue::Int32(x)   => x.to_string(),
                AnyValue::Int64(x)   => x.to_string(),
                AnyValue::UInt32(x)  => x.to_string(),
                AnyValue::UInt64(x)  => x.to_string(),
                AnyValue::Float32(x) => format!("{}", x),
                AnyValue::Float64(x) => format!("{}", x),
                other                => other.to_string(),
            })
            .collect::<Vec<_>>()
            .join(",");
        rows.push(format!("{}\n", formatted));
    }

    println!("  Ready: {} rows, timestamps {} → {}",
        n, timestamps.first().unwrap_or(&0), timestamps.last().unwrap_or(&0));

    Ok(CsvData { header, rows, timestamps })
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let cfg = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}\nRun with --help for usage.", e);
            std::process::exit(1);
        }
    };

    let shutdown = Arc::new(AtomicBool::new(false));
    let sd = Arc::clone(&shutdown);
    ctrlc::set_handler(move || {
        println!("\nCtrl+C received — stopping after current row…");
        sd.store(true, Ordering::SeqCst);
    }).expect("Could not set Ctrl+C handler");

    if let Err(e) = run(cfg, shutdown) {
        eprintln!("Fatal: {}", e);
        std::process::exit(1);
    }
}

fn run(cfg: Config, shutdown: Arc<AtomicBool>) -> Result<(), String> {
    let data = load_csv(&cfg.file)?;

    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Could not bind UDP socket: {}", e))?;
    socket.connect(&cfg.target)
        .map_err(|e| format!("Could not connect to {}: {}", cfg.target, e))?;

    let data_sim_secs = cfg.file.to_string_lossy().to_string();
    let sim_duration = data.timestamps.last().unwrap_or(&0)
        - data.timestamps.first().unwrap_or(&0);

    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  UDP Streamer");
    println!("  File     : {}", data_sim_secs);
    println!("  Target   : {}", cfg.target);
    println!("  Rows     : {}", data.rows.len());
    println!("  Sim span : {}s ({:.1} min)", sim_duration, sim_duration as f64 / 60.0);
    println!("  Speed    : {}×  →  real duration ~{:.0}s",
        cfg.speed,
        sim_duration as f64 / cfg.speed);
    println!("  Loop     : {}", if cfg.loop_mode { "yes" } else { "no" });
    println!("  Header   : {}", if cfg.send_header { "yes" } else { "no" });
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    let mut total_sent: u64 = 0;
    let mut pass: u32 = 0;
    let wall_start = Instant::now();

    'outer: loop {
        pass += 1;
        if pass > 1 {
            println!("  [loop {}]", pass);
        }

        // Optionally send the header as the first packet of each pass.
        // Prefix with "#HEADER:" so receivers can distinguish from data rows
        // that happen to contain non-numeric text fields.
        if cfg.send_header {
            let header_line = format!("#HEADER:{}\n", data.header);
            send_packet(&socket, header_line.as_bytes())?;
        }
        let mut rows_since_header: u64 = 0;

        let mut row_iter = data.rows.iter().zip(data.timestamps.iter()).peekable();
        let pass_wall_start = Instant::now();
        // Sim time offset: difference between first timestamp in file and
        // "now" in sim-time, anchored to wall clock at pass start.
        let first_ts = *data.timestamps.first().unwrap_or(&0);

        while let Some((row, &ts)) = row_iter.next() {
            if shutdown.load(Ordering::SeqCst) { break 'outer; }

            // How far into the sim are we? Wall time × speed = sim time elapsed.
            let wall_elapsed = pass_wall_start.elapsed().as_secs_f64();
            let sim_elapsed = wall_elapsed * cfg.speed;
            let row_sim_offset = (ts - first_ts) as f64;

            // If this row is ahead of where we are in sim time, sleep the gap.
            if row_sim_offset > sim_elapsed {
                let sleep_sim = row_sim_offset - sim_elapsed;
                let sleep_wall = Duration::from_secs_f64(sleep_sim / cfg.speed);

                // Sleep in 50ms chunks so Ctrl+C stays responsive
                let wake = Instant::now() + sleep_wall;
                while Instant::now() < wake {
                    if shutdown.load(Ordering::SeqCst) { break 'outer; }
                    let remaining = wake.saturating_duration_since(Instant::now());
                    std::thread::sleep(remaining.min(Duration::from_millis(50)));
                }
            }

            // Periodically resend header so late-joining receivers get column names.
            if cfg.send_header {
                rows_since_header += 1;
                if rows_since_header >= 500 {
                    let header_line = format!("#HEADER:{}\n", data.header);
                    let _ = send_packet(&socket, header_line.as_bytes());
                    rows_since_header = 0;
                }
            }

            send_packet(&socket, row.as_bytes())?;
            total_sent += 1;

            // Progress every 1000 rows
            if total_sent % 1000 == 0 {
                let real_elapsed = wall_start.elapsed().as_secs_f64();
                let rate = total_sent as f64 / real_elapsed;
                println!("  {:>8} rows sent  |  sim t={:<12}  |  {:.0} rows/sec  |  {:.0}s elapsed",
                    total_sent, ts, rate, real_elapsed);
            }
        }

        if !cfg.loop_mode { break; }
    }

    let elapsed = wall_start.elapsed().as_secs_f64();
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  Done.");
    println!("  Rows sent : {}", total_sent);
    println!("  Passes    : {}", pass);
    println!("  Elapsed   : {:.1}s", elapsed);
    if elapsed > 0.0 {
        println!("  Avg rate  : {:.0} rows/sec", total_sent as f64 / elapsed);
    }
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    Ok(())
}

#[inline]
fn send_packet(socket: &UdpSocket, data: &[u8]) -> Result<(), String> {
    socket.send(data).map_err(|e| format!("UDP send error: {}", e))?;
    Ok(())
}
