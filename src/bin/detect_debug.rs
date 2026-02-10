use std::path::Path;

fn main() {
    let db_path = Path::new(".codemcp/codegraph.db");

    // Read header directly
    match std::fs::read(db_path) {
        Ok(data) => {
            let header = &data[..std::cmp::min(16, data.len())];
            println!("File header bytes: {:?}", header);
            println!("Header as string: {:?}", String::from_utf8_lossy(header));
        }
        Err(e) => println!("Cannot read file: {}", e),
    }

    // Try magellan detection
    #[cfg(feature = "native-v2")]
    {
        match magellan::migrate_backend_cmd::detect_backend_format(db_path) {
            Ok(format) => println!("Magellan detected: {:?}", format),
            Err(e) => println!("Magellan detection error: {}", e),
        }
    }
}
