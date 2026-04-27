use std::path::Path;

fn main() {
    let db_path = Path::new(".magellan/llmgrep.db");

    // Read header directly
    match std::fs::read(db_path) {
        Ok(data) => {
            let header = &data[..std::cmp::min(16, data.len())];
            println!("File header bytes: {:?}", header);
            println!("Header as string: {:?}", String::from_utf8_lossy(header));
        }
        Err(e) => println!("Cannot read file: {}", e),
    }
}
