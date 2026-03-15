use anyhow::Result;

use crate::external::{flatpak, protontricks};

const PROTONUP_QT_FLATPAK: &str = "net.davidotek.pupgui2";

/// Ensure protontricks is installed, installing it if needed.
pub async fn ensure_protontricks() -> Result<()> {
    if protontricks::is_available().await {
        tracing::info!("protontricks is available");
        return Ok(());
    }

    tracing::info!("Installing protontricks...");
    protontricks::install().await
}

/// Check if ProtonUp-Qt is available.
pub async fn is_protonup_available() -> bool {
    flatpak::is_flatpak_installed(PROTONUP_QT_FLATPAK).await
}

/// Install ProtonUp-Qt if not already installed.
pub async fn ensure_protonup() -> Result<()> {
    if is_protonup_available().await {
        return Ok(());
    }
    flatpak::install_flatpak(PROTONUP_QT_FLATPAK).await
}

/// Launch ProtonUp-Qt.
pub async fn launch_protonup() -> Result<()> {
    flatpak::launch_flatpak(PROTONUP_QT_FLATPAK, &[]).await
}

/// Install .NET Framework 4.8 for the given game's prefix.
pub async fn install_dotnet(app_id: u32) -> Result<()> {
    protontricks::install_dotnet(app_id).await
}
