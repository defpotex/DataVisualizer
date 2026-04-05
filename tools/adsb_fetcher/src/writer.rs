use crate::record::AircraftState;
use polars::prelude::*;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// Manages the output CSV file — writes header on first open,
/// then appends rows on each poll.
/// Named OutputCsvWriter to avoid collision with polars::prelude::CsvWriter.
pub struct OutputCsvWriter {
    path: PathBuf,
    writer: csv::Writer<BufWriter<File>>,
    row_count: u64,
}

#[allow(dead_code)]
impl OutputCsvWriter {
    /// Open or create the CSV file. Writes the header row immediately.
    pub fn create(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Could not create output directory: {}", e))?;
        }

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .map_err(|e| format!("Could not open {}: {}", path.display(), e))?;

        let buf = BufWriter::new(file);
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false) // we write the header manually
            .from_writer(buf);

        // Write header
        writer
            .write_record(AircraftState::csv_headers())
            .map_err(|e| format!("CSV header write error: {}", e))?;

        Ok(OutputCsvWriter { path: path.to_path_buf(), writer, row_count: 0 })
    }

    /// Append a batch of records. Returns the number of rows written.
    pub fn append(&mut self, records: &[AircraftState]) -> Result<u64, String> {
        for record in records {
            self.writer
                .write_record(&record.to_csv_row())
                .map_err(|e| format!("CSV write error: {}", e))?;
        }
        self.row_count += records.len() as u64;
        Ok(records.len() as u64)
    }

    /// Flush all buffered data to disk.
    pub fn flush(&mut self) -> Result<(), String> {
        self.writer.flush().map_err(|e| format!("CSV flush error: {}", e))
    }

    pub fn row_count(&self) -> u64 {
        self.row_count
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Read the completed CSV, sort by timestamp, re-write it, then
/// produce a sibling .parquet file. Returns (sorted_rows, parquet_path).
pub fn finalize(csv_path: &Path) -> Result<(u64, PathBuf), String> {
    println!("  Sorting by timestamp…");

    // Read via polars lazy API
    let df = LazyCsvReader::new(csv_path)
        .with_has_header(true)
        .finish()
        .map_err(|e| format!("Could not read CSV for sorting: {}", e))?
        .sort(
            ["timestamp"],
            SortMultipleOptions::default().with_order_descending(false),
        )
        .collect()
        .map_err(|e| format!("Sort failed: {}", e))?;

    let row_count = df.height() as u64;

    // Re-write sorted CSV (polars::prelude::CsvWriter)
    let csv_file = File::create(csv_path)
        .map_err(|e| format!("Could not rewrite CSV: {}", e))?;
    CsvWriter::new(csv_file)
        .finish(&mut df.clone())
        .map_err(|e| format!("CSV re-write error: {}", e))?;

    // Write Parquet sibling
    let parquet_path = csv_path.with_extension("parquet");
    let parquet_file = File::create(&parquet_path)
        .map_err(|e| format!("Could not create parquet file: {}", e))?;
    ParquetWriter::new(parquet_file)
        .with_compression(ParquetCompression::Snappy)
        .finish(&mut df.clone())
        .map_err(|e| format!("Parquet write error: {}", e))?;

    Ok((row_count, parquet_path))
}

/// Write a small JSON metadata sidecar.
pub fn write_meta(
    meta_path: &Path,
    row_count: u64,
    time_start: i64,
    time_end: i64,
    bbox_str: &str,
    poll_count: u32,
) -> Result<(), String> {
    let meta = serde_json::json!({
        "row_count": row_count,
        "poll_count": poll_count,
        "time_start_unix": time_start,
        "time_end_unix": time_end,
        "bbox": bbox_str,
        "columns": AircraftState::csv_headers(),
    });

    let mut f = File::create(meta_path)
        .map_err(|e| format!("Could not write meta: {}", e))?;
    f.write_all(serde_json::to_string_pretty(&meta).unwrap().as_bytes())
        .map_err(|e| format!("Meta write error: {}", e))?;
    Ok(())
}
