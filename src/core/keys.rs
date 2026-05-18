use anyhow::Context;
use rand::RngExt;
use russh::keys::ssh_key::private::{Ed25519Keypair, Ed25519PrivateKey, KeypairData};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Load an Ed25519 host key from `path`, generating + persisting a fresh one
/// if the file is missing or unreadable. The bytes on disk are whatever
/// `russh::keys::PrivateKey::to_bytes()` produces.
pub fn load_or_generate<P: AsRef<Path>>(path: P) -> anyhow::Result<russh::keys::PrivateKey> {
    let path = path.as_ref();
    match load(path) {
        Ok(k) => {
            log::info!("Loaded keypair from {}.", path.display());
            Ok(k)
        }
        Err(_) => {
            let key = generate("ssh game server key")?;
            save(path, &key)
                .with_context(|| format!("saving generated key to {}", path.display()))?;
            log::info!("Generated new keypair at {}.", path.display());
            Ok(key)
        }
    }
}

fn load(path: &Path) -> anyhow::Result<russh::keys::PrivateKey> {
    let bytes = std::fs::read(path)?;
    Ok(russh::keys::PrivateKey::from_bytes(&bytes)?)
}

fn save(path: &Path, key: &russh::keys::PrivateKey) -> anyhow::Result<()> {
    let mut buffer = std::io::BufWriter::new(File::create(path)?);
    buffer.write_all(&key.to_bytes()?)?;
    Ok(())
}

fn generate(comment: &str) -> anyhow::Result<russh::keys::PrivateKey> {
    let seed: [u8; Ed25519PrivateKey::BYTE_SIZE] = rand::rng().random();
    let key_data = KeypairData::from(Ed25519Keypair::from_seed(&seed));
    Ok(russh::keys::PrivateKey::new(key_data, comment)?)
}
