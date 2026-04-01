use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <encrypted_dir>", args[0]);
        std::process::exit(1);
    }

    let dir_path = Path::new(&args[1]);
    if !dir_path.is_dir() {
        eprintln!("Error: {} is not a directory", args[1]);
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

    // Old format: 19 bytes nonce + chunks
    // New format: 1 byte cipher type + 19 bytes nonce + chunks
    // If length is less than 19, it might not be encrypted in the old format,
    // but the instructions say to convert it.

    // We'll read the whole file, prepend the header, and write it back.
    // For very large files, this is not efficient, but it's a small side binary.
    // Let's do it by creating a temporary file to be safer.

    if len == 0 {
        // Skip empty files or handle them if needed.
        // Based on DavFile::open, empty files don't have nonces.
        return Ok(());
    }

    println!("Converting: {:?}", path);

    let mut content = Vec::new();
    file.read_to_end(&mut content)?;

    let mut new_path = path.to_path_buf();
    new_path.set_extension("tmp_convert");

    {
        let mut new_file = File::create(&new_path)?;
        let cipher_type: u8 = 0; // XChaCha20Poly1305_1M

        new_file.write_all(&[cipher_type])?;
        new_file.write_all(&content)?;
    }

    fs::rename(&new_path, path)?;

    Ok(())
}
