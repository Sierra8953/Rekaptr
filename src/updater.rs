use axoupdater::AxoUpdater;

const APP_NAME: &str = "rekaptr";

#[derive(Clone, Debug)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available { new_version: String },
    Installing,
    Installed { new_version: String },
    Error(String),
}

impl Default for UpdateState {
    fn default() -> Self { UpdateState::Idle }
}

pub fn has_install_receipt() -> bool {
    let mut updater = AxoUpdater::new_for(APP_NAME);
    updater.load_receipt().is_ok()
}

pub fn check_for_update() -> Result<Option<String>, String> {
    let mut updater = AxoUpdater::new_for(APP_NAME);
    updater.load_receipt().map_err(|e| format!("no install receipt: {e}"))?;

    if !updater.is_update_needed_sync().map_err(|e| e.to_string())? {
        return Ok(None);
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let new_version = rt
        .block_on(updater.query_new_version())
        .map_err(|e| e.to_string())?
        .map(|v| v.to_string());
    Ok(new_version)
}

pub fn install_update() -> Result<Option<String>, String> {
    let mut updater = AxoUpdater::new_for(APP_NAME);
    updater.load_receipt().map_err(|e| format!("no install receipt: {e}"))?;
    let result = updater.run_sync().map_err(|e| e.to_string())?;
    Ok(result.map(|r| r.new_version.to_string()))
}
