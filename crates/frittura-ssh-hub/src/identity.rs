use crate::AppResult;
use anyhow::Context;
use rand::RngExt;
use russh::keys::ssh_key::private::{Ed25519Keypair, KeypairData};
use russh::keys::PrivateKey;
use std::fs;
use std::io::Write;
use std::path::Path;

const MASTER_SIZE: usize = 32;
const DERIVE_DOMAIN: &str = "frittura-hub/v1";

pub struct HubIdentity {
    master: [u8; MASTER_SIZE],
}

impl HubIdentity {
    pub fn load_or_generate(path: &Path) -> AppResult<Self> {
        if let Ok(bytes) = fs::read(path) {
            if bytes.len() == MASTER_SIZE {
                let mut master = [0u8; MASTER_SIZE];
                master.copy_from_slice(&bytes);
                log::info!("Loaded hub identity master from {}.", path.display());
                return Ok(Self { master });
            }
            log::warn!(
                "Hub identity master at {} has wrong length ({}), regenerating.",
                path.display(),
                bytes.len()
            );
        }
        let master: [u8; MASTER_SIZE] = rand::rng().random();
        let mut f = fs::File::create(path)
            .with_context(|| format!("creating hub identity master at {}", path.display()))?;
        f.write_all(&master)?;
        f.sync_all()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        }
        log::info!("Generated new hub identity master at {}.", path.display());
        Ok(Self { master })
    }

    pub fn derive_for(&self, game_key: &str, identifier: &str) -> AppResult<PrivateKey> {
        let mut ctx = Vec::with_capacity(DERIVE_DOMAIN.len() + game_key.len() + identifier.len() + 2);
        ctx.extend_from_slice(DERIVE_DOMAIN.as_bytes());
        ctx.push(0);
        ctx.extend_from_slice(game_key.as_bytes());
        ctx.push(0);
        ctx.extend_from_slice(identifier.as_bytes());
        let seed = blake3::keyed_hash(&self.master, &ctx);
        let kp = Ed25519Keypair::from_seed(seed.as_bytes());
        Ok(PrivateKey::new(KeypairData::from(kp), "frittura-hub derived")?)
    }
}
