use std::{
    path::Path,
    process::{Command, Stdio},
};

use super::HelperFailure;

pub(super) fn install(root: &Path, package: &Path) -> Result<(), HelperFailure> {
    let loader = root.join("lib/ld-musl-x86_64.so.1");
    let apk = root.join("sbin/apk");
    for path in [loader.as_path(), apk.as_path(), package] {
        if !path
            .metadata()
            .map_err(|_| HelperFailure::BootstrapRootVerification)?
            .is_file()
        {
            return Err(HelperFailure::BootstrapRootVerification);
        }
    }
    // The private root intentionally lives on tmpfs. This option skips only
    // APK's reboot-persistence guard; package signature verification still runs.
    let status = Command::new(loader)
        .arg(apk)
        .args(["add", "--root"])
        .arg(root)
        .args(["--no-cache", "--no-network", "--force-non-repository"])
        .arg(package)
        .env_clear()
        .env(
            "LD_LIBRARY_PATH",
            format!("{}/lib:{}/usr/lib", root.display(), root.display()),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    if status.success() {
        Ok(())
    } else {
        Err(HelperFailure::BootstrapRootVerification)
    }
}

#[cfg(test)]
mod tests;
