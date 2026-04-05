mod api;
mod record;
mod writer;

use api::BoundingBox;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ── CLI argument parsing (no external dep — simple hand-rolled) ───────────────

struct Config {
    output_dir: PathBuf,
    duration_mins: u64,   // 0 = run until Ctrl+C
    interval_secs: u64,
    bbox: BoundingBox,
    name: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("test_data"),
            duration_mins: 60,
            interval_secs: 60,
            bbox: BoundingBox::conus(),
            name: "adsb_conus".to_string(),
        }
    }
}

fn parse_args() -> Result<Config, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut cfg = Config::default();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            "--output" => {
                i += 1;
                cfg.output_dir = PathBuf::from(args.get(i).ok_or("--output needs a value")?);
            }
            "--duration" => {
                i += 1;
                cfg.duration_mins = args.get(i).ok_or("--duration needs a value")?
                    .parse::<u64>().map_err(|_| "--duration must be a whole number")?;
            }
            "--interval" => {
                i += 1;
                cfg.interval_secs = args.get(i).ok_or("--interval needs a value")?
                    .parse::<u64>().map_err(|_| "--interval must be a whole number")?;
                if cfg.interval_secs < 10 {
                    return Err("--interval must be >= 10 seconds (OpenSky rate limit)".to_string());
                }
            }
            "--bbox" => {
                i += 1;
                cfg.bbox = BoundingBox::parse(args.get(i).ok_or("--bbox needs a value")?)?;
            }
            "--name" => {
                i += 1;
                cfg.name = args.get(i).ok_or("--name needs a value")?.clone();
            }
            other => return Err(format!("Unknown argument: {}", other)),
        }
        i += 1;
    }

    Ok(cfg)
}

fn print_usage() {
    println!(
        r#"adsb_fetcher — fetch ADS-B state vectors from OpenSky Network

USAGE:
  cargo run -p adsb_fetcher -- [OPTIONS]

OPTIONS:
  --output <DIR>       Output directory            [default: test_data/]
  --duration <MINS>    Collection duration (0=∞)   [default: 60]
  --interval <SECS>    Poll interval (min 10)      [default: 60]
  --bbox <W,S,E,N>     Bounding box                [default: CONUS -130,24,-60,50]
  --name <STEM>        Output filename stem        [default: adsb_conus]

OUTPUTS:
  <output>/<name>.csv
  <output>/<name>.parquet
  <output>/<name>.meta.json

EXAMPLES:
  cargo run -p adsb_fetcher
  cargo run -p adsb_fetcher -- --duration 120 --interval 30
  cargo run -p adsb_fetcher -- --bbox -80,25,-65,35 --name adsb_florida
"#
    );
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let cfg = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}\nRun with --help for usage.", e);
            std::process::exit(1);
        }
    };

    // Ctrl+C handler — sets flag, main loop checks each iteration
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_ctrlc = Arc::clone(&shutdown);
    ctrlc::set_handler(move || {
        println!("\nCtrl+C received — finishing current poll then writing output…");
        shutdown_ctrlc.store(true, Ordering::SeqCst);
    })
    .expect("Could not set Ctrl+C handler");

    if let Err(e) = run(cfg, shutdown) {
        eprintln!("Fatal error: {}", e);
        std::process::exit(1);
    }
}

fn run(cfg: Config, shutdown: Arc<AtomicBool>) -> Result<(), String> {
    let csv_path = cfg.output_dir.join(format!("{}.csv", cfg.name));
    let meta_path = cfg.output_dir.join(format!("{}.meta.json", cfg.name));

    let total_polls = if cfg.duration_mins == 0 {
        u32::MAX // run until Ctrl+C
    } else {
        // +1 so we always do at least one poll even if duration < interval
        ((cfg.duration_mins * 60).div_ceil(cfg.interval_secs) + 1)
            .min(u32::MAX as u64) as u32
    };

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  ADS-B Fetcher");
    println!("  Output : {}", csv_path.display());
    println!("  BBox   : W={} S={} E={} N={}",
        cfg.bbox.lon_min, cfg.bbox.lat_min, cfg.bbox.lon_max, cfg.bbox.lat_max);
    if cfg.duration_mins == 0 {
        println!("  Mode   : run until Ctrl+C");
    } else {
        println!("  Mode   : {} min, {} polls planned", cfg.duration_mins, total_polls);
    }
    println!("  Interval: {}s", cfg.interval_secs);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let mut csv = writer::OutputCsvWriter::create(&csv_path)?;

    let mut poll_num: u32 = 0;
    let mut time_first: i64 = 0;
    let mut time_last: i64 = 0;
    let mut total_rows: u64 = 0;

    while poll_num < total_polls && !shutdown.load(Ordering::SeqCst) {
        poll_num += 1;
        let poll_start = Instant::now();

        let polls_label = if total_polls == u32::MAX {
            format!("{}", poll_num)
        } else {
            format!("{}/{}", poll_num, total_polls)
        };
        print!("  Poll {} — fetching… ", polls_label);
        std::io::Write::flush(&mut std::io::stdout()).ok();

        match api::fetch_states(&cfg.bbox) {
            Ok((snapshot_time, records)) => {
                let n = records.len();
                csv.append(&records)?;
                total_rows += n as u64;

                if time_first == 0 { time_first = snapshot_time; }
                time_last = snapshot_time;

                println!(
                    "{} aircraft  |  {:>7} rows total  |  t={}",
                    n, total_rows, snapshot_time
                );
            }
            Err(e) => {
                println!("WARN: {} — skipping poll", e);
            }
        }

        csv.flush()?;

        // Sleep for the remainder of the interval, checking shutdown flag
        // every second so Ctrl+C feels responsive.
        let elapsed = poll_start.elapsed();
        let interval = Duration::from_secs(cfg.interval_secs);
        if elapsed < interval && poll_num < total_polls {
            let remaining = interval - elapsed;
            let wake_at = Instant::now() + remaining;
            while Instant::now() < wake_at {
                if shutdown.load(Ordering::SeqCst) { break; }
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }

    if total_rows == 0 {
        println!("\nNo rows collected — nothing to write.");
        return Ok(());
    }

    // Finalize: sort + write Parquet
    println!("\n  Finalizing output ({} rows)…", total_rows);
    let (sorted_rows, parquet_path) = writer::finalize(&csv_path)?;

    writer::write_meta(
        &meta_path,
        sorted_rows,
        time_first,
        time_last,
        &format!(
            "W={},S={},E={},N={}",
            cfg.bbox.lon_min, cfg.bbox.lat_min, cfg.bbox.lon_max, cfg.bbox.lat_max
        ),
        poll_num,
    )?;

    // File size helpers
    let csv_mb = file_mb(&csv_path);
    let parquet_mb = file_mb(&parquet_path);

    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  Done!");
    println!("  Rows     : {}", sorted_rows);
    println!("  Polls    : {}", poll_num);
    println!("  CSV      : {} ({:.1} MB)", csv_path.display(), csv_mb);
    println!("  Parquet  : {} ({:.1} MB)", parquet_path.display(), parquet_mb);
    println!("  Metadata : {}", meta_path.display());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    Ok(())
}

fn file_mb(path: &std::path::Path) -> f64 {
    std::fs::metadata(path)
        .map(|m| m.len() as f64 / 1_048_576.0)
        .unwrap_or(0.0)
}
