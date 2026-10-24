use std::{env, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-env-changed=RECYCLE_DESK_REPOSITORY");
    println!("cargo:rerun-if-env-changed=GITHUB_REPOSITORY");
    println!("cargo:rerun-if-changed=resources/update-public.key");
    let repository = env::var("RECYCLE_DESK_REPOSITORY")
        .or_else(|_| env::var("GITHUB_REPOSITORY"))
        .unwrap_or_default();
    println!("cargo:rustc-env=RECYCLE_DESK_REPOSITORY={repository}");
    match std::fs::read("resources/update-public.key") {
        Ok(key) => {
            assert_eq!(key.len(), 32, "Update public key must contain 32 bytes");
            let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
            println!("cargo:rustc-env=RECYCLE_DESK_VERIFYING_KEY={hex}");
        }
        Err(_) if repository.is_empty() => {}
        Err(e) => panic!("Release repository configured without an update public key: {e}"),
    }
    println!("cargo:rerun-if-changed=resources/app.rc");
    println!("cargo:rerun-if-changed=resources/app.manifest");
    println!("cargo:rerun-if-changed=resources/icons/recycle-desk-v1.ico");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let tuple = format!("{},0", version.replace('.', ","));
    let rc = std::fs::read_to_string("resources/app.rc")
        .unwrap()
        .replace("0,1,0,0", &tuple)
        .replace("\"0.1.0\"", &format!("\"{version}\""));
    let resource_source = out.join("app.rc");
    std::fs::write(&resource_source, rc).unwrap();
    let target = env::var("TARGET").unwrap();
    let (mut command, resource) = if target.ends_with("-gnu") {
        let output = out.join("app.o");
        let mut cmd = Command::new("windres");
        cmd.arg("-i")
            .arg(&resource_source)
            .args(["-O", "coff", "-o"])
            .arg(&output);
        (cmd, output)
    } else {
        let output = out.join("app.res");
        let sdk = PathBuf::from(
            env::var_os("ProgramFiles(x86)").unwrap_or_else(|| "C:/Program Files (x86)".into()),
        )
        .join("Windows Kits/10/bin");
        let mut versions: Vec<_> = std::fs::read_dir(sdk)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        versions.sort();
        let compiler = env::var_os("RC").map(PathBuf::from).or_else(|| {
            versions
                .iter()
                .rev()
                .map(|p| p.join("x64/rc.exe"))
                .find(|p| p.exists())
        });
        let mut cmd = Command::new(compiler.unwrap_or_else(|| "rc.exe".into()));
        cmd.args(["/nologo", "/fo"])
            .arg(&output)
            .arg(&resource_source);
        (cmd, output)
    };
    let status = command
        .current_dir("resources")
        .status()
        .expect("Windows resource compiler not found (rc.exe or windres)");
    assert!(status.success(), "Windows resource compilation failed");
    println!("cargo:rustc-link-arg={}", resource.display());
}
