use crate::data::schema::DataSchema;
use crate::data::source::DataSource;
use crate::state::app_state::DataEvent;
use crossbeam_channel::Sender;
use polars::prelude::*;
use std::path::PathBuf;

/// Spawn a background thread that loads a CSV file and sends the result
/// back to the UI thread via the provided channel.
///
/// The UI thread remains fully responsive while loading.
pub fn load_csv_async(id: usize, path: PathBuf, tx: Sender<DataEvent>) {
    std::thread::spawn(move || {
        let result = load_csv(id, &path);
        // If send fails the receiver has been dropped — just ignore.
        let _ = tx.send(match result {
            Ok(source) => DataEvent::Loaded(source),
            Err(e)     => DataEvent::LoadError { id, message: e },
        });
    });
}

fn load_csv(id: usize, path: &PathBuf) -> Result<DataSource, String> {
    let df = LazyCsvReader::new(path)
        .with_has_header(true)
        .with_try_parse_dates(true)
        .finish()
        .map_err(|e| format!("Could not read CSV: {}", e))?
        .collect()
        .map_err(|e| format!("Could not load CSV into memory: {}", e))?;

    let schema = DataSchema::infer(&df);

    let label = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("source_{}", id));

    Ok(DataSource {
        id,
        label,
        path: Some(path.clone()),
        schema,
        df,
    })
}
