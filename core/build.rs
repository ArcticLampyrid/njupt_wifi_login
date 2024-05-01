use std::fs;
use std::process::Command;

fn main() {
    // Embed the Windows resource file
    if let Ok(paths) = fs::read_dir("win32_resource") {
        for path in paths.flatten() {
            println!("cargo:rerun-if-changed={}", path.path().display());
        }
    }

    let git_describe = Command::new("git")
        .args(["describe", "--tags", "--dirty", "--always"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .and_then(|git_describe| git_describe.trim().strip_prefix('v').map(|s| s.to_string()));
    let version_full = git_describe.as_deref().unwrap_or(env!("CARGO_PKG_VERSION"));

    let marcos = &[
        format!("VERSION_FULL=\"{}\"", version_full),
        format!("VERSION_MAJOR={}", env!("CARGO_PKG_VERSION_MAJOR")),
        format!("VERSION_MINOR={}", env!("CARGO_PKG_VERSION_MINOR")),
        format!("VERSION_PATCH={}", env!("CARGO_PKG_VERSION_PATCH")),
    ];

    embed_resource::compile("win32_resource/app.rc", marcos);
}
