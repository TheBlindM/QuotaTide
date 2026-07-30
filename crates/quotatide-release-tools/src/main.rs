use std::error::Error;
use std::io::{Error as IoError, ErrorKind};
use std::path::Path;

use base64::Engine as _;
use minisign_verify::{PublicKey, Signature};

fn decode_tauri_text(value: &str, label: &str) -> Result<String, Box<dyn Error>> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(value.trim())
        .map_err(|_| IoError::new(ErrorKind::InvalidData, format!("invalid {label} encoding")))?;
    String::from_utf8(decoded)
        .map_err(|_| IoError::new(ErrorKind::InvalidData, format!("invalid {label} text")).into())
}

fn verify(
    artifact_path: &Path,
    signature_path: &Path,
    public_key_path: &Path,
) -> Result<(), Box<dyn Error>> {
    let artifact = std::fs::read(artifact_path)?;
    let encoded_signature = std::fs::read_to_string(signature_path)?;
    let encoded_public_key = std::fs::read_to_string(public_key_path)?;
    let signature =
        Signature::decode(&decode_tauri_text(&encoded_signature, "updater signature")?)?;
    let public_key = PublicKey::decode(&decode_tauri_text(
        &encoded_public_key,
        "updater public key",
    )?)?;
    public_key.verify(&artifact, &signature, true)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    if arguments.len() != 4 {
        return Err(IoError::new(
            ErrorKind::InvalidInput,
            "usage: quotatide-release-tools <artifact> <signature> <public-key>",
        )
        .into());
    }
    verify(
        Path::new(&arguments[1]),
        Path::new(&arguments[2]),
        Path::new(&arguments[3]),
    )?;
    println!("Tauri updater signature verified against final artifact bytes");
    Ok(())
}
