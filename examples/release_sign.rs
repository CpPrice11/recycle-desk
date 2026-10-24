//! Developer-only signing tool. Never prints private key material.
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
};
use zipsign_api::SigningKey;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    match args.get(1).and_then(|s| s.to_str()) {
        Some("gen-key") if args.len() == 4 => {
            let private = Path::new(&args[2]);
            let public = Path::new(&args[3]);
            if private.exists() || public.exists() {
                return Err("Refusing to overwrite signing keys".into());
            }
            let mut seed = [0u8; 32];
            unsafe {
                windows::Win32::Security::Cryptography::BCryptGenRandom(
                    None,
                    &mut seed,
                    windows::Win32::Security::Cryptography::BCRYPT_USE_SYSTEM_PREFERRED_RNG,
                )
                .ok()?;
            }
            let key = SigningKey::from_bytes(&seed);
            if let Some(parent) = private.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut secret = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(private)?;
            secret.write_all(&key.to_keypair_bytes())?;
            secret.sync_all()?;
            let mut output = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(public)?;
            output.write_all(&key.verifying_key().to_bytes())?;
            output.sync_all()?;
            println!("Signing key created. Only the public key belongs in the repository.");
        }
        Some("sign") if args.len() == 5 => {
            let keydata: [u8; 64] = fs::read(&args[2])?
                .try_into()
                .map_err(|_| "Invalid private key length")?;
            let key = SigningKey::from_keypair_bytes(&keydata)?;
            let public = fs::read("resources/update-public.key")?;
            if public != key.verifying_key().to_bytes() {
                return Err("Signing key does not match pinned public key".into());
            }
            let mut source = File::open(&args[3])?;
            let destination = Path::new(&args[4]);
            let context = destination
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or("Invalid signed archive name")?;
            let mut output = OpenOptions::new()
                .create_new(true)
                .read(true)
                .write(true)
                .open(destination)?;
            zipsign_api::sign::copy_and_sign_zip(
                &mut source,
                &mut output,
                &[key],
                Some(context.as_bytes()),
            )?;
            output.sync_all()?;
            self_update::verify_signature(
                destination,
                &[public.try_into().map_err(|_| "Invalid public key length")?],
            )?;
            println!("Signed update archive verified.");
        }
        _ => {
            return Err(
                "Usage: release_sign gen-key PRIVATE PUBLIC | sign PRIVATE INPUT.zip OUTPUT.zip"
                    .into(),
            )
        }
    }
    Ok(())
}
