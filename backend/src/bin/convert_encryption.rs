use filetime::{FileTime, set_file_times};
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <encrypted_dir>", args.first().expect("args"));
        std::process::exit(1);
    }

    let dir_path = Path::new(args.get(1).expect("args"));
    if !dir_path.is_dir() {
        eprintln!("Error: {} is not a directory", args.get(1).expect("args"));
        std::process::exit(1);
    }

    convert_dir(dir_path)?;

    println!("Conversion completed successfully.");
    Ok(())
}

fn convert_dir(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            convert_dir(&path)?;
        } else if path.is_file() {
            convert_file(&path)?;
        }
    }
    Ok(())
}

fn convert_file(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    let len = metadata.len();

    if len < 19 && len > 0 {
        eprintln!(
            "ALERT: File {:?} is too small to be a valid old-format encrypted file (size: {} bytes)",
            path, len
        );
    }

    // Old format: 19 bytes nonce + chunks
    // New format: 1 byte cipher type + 19 bytes nonce + chunks

    if len == 0 {
        return Ok(());
    }

    // Capture metadata
    let atime = FileTime::from_last_access_time(&metadata);
    let mtime = FileTime::from_last_modification_time(&metadata);

    println!("Converting: {:?}", path);

    let mut new_path = path.to_path_buf();
    let original_file_name = path.file_name().ok_or("Invalid file name")?;
    let mut new_file_name = original_file_name.to_os_string();
    new_file_name.push(".tmp_convert");
    new_path.set_file_name(new_file_name);

    {
        let mut new_file = File::create(&new_path)?;
        let cipher_type: u8 = 0; // XChaCha20Poly1305_1M

        new_file.write_all(&[cipher_type])?;
        std::io::copy(&mut file, &mut new_file)?;
    }

    fs::rename(&new_path, path)?;

    // Restore metadata
    set_file_times(path, atime, mtime)?;

    Ok(())
}
