use crate::{
    app_state::{AppStateManager, Progress},
    Result, SDCARD_ROOT,
};
use bytes::Bytes;
use fetching::{download, fetch_latest_release, fetch_tag};
use zip::read::root_dir_common_filter;

use std::{
    fs::File,
    io::{Cursor, Read, Write},
    process::exit,
    thread,
};

mod fetching;

fn extract_zip(bytes: Bytes, do_root_dir: bool, _progress_cb: impl Fn(f32)) -> Result<()> {
    // Extract the update package
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;

    if do_root_dir {
        archive.extract_unwrapped_root_dir(SDCARD_ROOT, root_dir_common_filter)
    } else {
        archive.extract(SDCARD_ROOT)
    }?;

    Ok(())
}

pub fn self_update(app_state: &AppStateManager) -> Result<()> {
    // Fetch latest release information
    app_state.start_operation("Fetching latest updater release...");

    println!("Fetching latest updater release...");

    let release = fetch_latest_release("LanderN/nextui-updater-pak")?;

    println!("Latest updater release: {release:?}");

    let available = semver::Version::parse(&release.tag_name)?;
    let installed = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;

    if available > installed {
        println!("New version available: {available} (current: {installed})");
        app_state.set_current_operation(Some("Downloading updater...".to_string()));
    } else {
        println!("No updates available");
        return Ok(());
    }

    let bytes = download(&release.assets[0].url, |pr| {
        app_state.update_progress(pr);
    })?;

    app_state
        .set_current_operation(format!("Extracting NextUI Updater {}...", release.tag_name).into());
    app_state.set_progress(Some(Progress::Indeterminate));

    // Move the current binary to a backup location
    let current_binary = std::env::current_exe()?;
    std::fs::rename(&current_binary, current_binary.with_extension("bak"))?;

    // Extract the update package
    let result = extract_zip(bytes, false, |pr| {
        app_state.update_progress(pr);
    });

    if result.is_err() {
        // Move the backup back
        std::fs::rename(current_binary.with_extension("bak"), current_binary)?;

        return Err("Failed to extract update package".into());
    }

    app_state.set_current_operation(Some(
        "Self-update success! Restarting updater...".to_string(),
    ));

    // Give the user a moment to see the completion message
    thread::sleep(std::time::Duration::from_secs(1));

    // "5" is the exit code for "restart required"
    exit(5);
}

pub fn do_nextui_release_check(app_state: &AppStateManager) {
    // Fetch latest release information
    app_state.start_operation("Fetching latest NextUI release...");

    let latest_release = fetch_latest_release("LoveRetro/NextUI");

    match &latest_release {
        Ok(release) => {
            app_state.set_nextui_release(Some(release.clone()));
        }
        Err(err) => {
            println!("Release fetch failed: {:?}", err.source());
            app_state.set_operation_failed(&format!("Release fetch failed: {err}"));
        }
    }

    if latest_release.is_err() {
        return;
    }
    let latest_release = latest_release.unwrap();

    // Fetch latest tag information
    app_state.start_operation("Fetching latest NextUI tag...");

    let latest_tag = fetch_tag("LoveRetro/NextUI", &latest_release.tag_name);
    match latest_tag {
        Ok(tag) => {
            app_state.set_nextui_tag(Some(tag.clone()));
        }
        Err(err) => {
            println!("Tag fetch failed: {:?}", err.source());
            app_state.set_operation_failed(&format!("Tag fetch failed: {err}"));
        }
    }

    app_state.finish_operation();
}

pub fn do_self_update(app_state: &AppStateManager) {
    // Do self-update
    let result = self_update(app_state);
    match result {
        Ok(()) => {
            app_state.finish_operation();
        }
        Err(err) => {
            println!("Self-update failed: {:?}", err.source());
            app_state.set_operation_failed(&format!("Self-update failed: {err}"));
        }
    }
}

pub fn do_update(app_state: &'static AppStateManager, full: bool) {
    thread::spawn(move || {
        if let Err(err) = update_nextui(app_state, full) {
            println!("Update failed: {:?}", err.source());

            app_state.set_operation_failed(&format!("Update failed: {err}"));

            // Try to fetch latest release information again
            do_nextui_release_check(app_state);
        }
    });
}

pub fn update_nextui(app_state: &AppStateManager, full: bool) -> Result<()> {
    let release = {
        app_state.start_operation("Downloading update...");

        app_state
            .nextui_release()
            .clone()
            .ok_or("No release found")?
    };

    let assets = release.assets;
    let asset = assets
        .iter()
        .find(|a| a.name.contains(if full { "all" } else { "base" }))
        .or(assets.first())
        .ok_or("No assets found")?;

    // Download the asset
    app_state.start_determinate_operation(&format!("Downloading {}...", asset.name));
    println!("Downloading from {}", asset.url);

    let bytes = download(&asset.url, |pr| app_state.update_progress(pr))?;

    app_state.set_current_operation(format!("Extracting {}...\nPlease wait...", asset.name).into());
    app_state.set_progress(Some(Progress::Indeterminate));

    // Extract the update package
    if full {
        // Full update, extract all files
        extract_zip(bytes, false, |pr| app_state.update_progress(pr))?;
    } else {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
        // "Quick" update, just extract MinUI.zip

        // Look for MinUI.zip in the archive
        let mut minui_data = Vec::new();
        match archive.by_name("MinUI.zip") {
            Ok(mut file) => {
                file.read_to_end(&mut minui_data)?;
            }
            Err(_) => return Err("File MinUI.zip not found in archive".into()),
        }

        // Write the extracted file
        let mut file = File::create([SDCARD_ROOT, "MinUI.zip"].join("/"))?;
        file.write_all(&minui_data)?;
    }

    app_state.set_current_operation(Some("Update complete, preparing to reboot...".to_string()));

    // Give the user a moment to see the completion message
    thread::sleep(std::time::Duration::from_secs(2));

    app_state.set_current_operation(Some("Rebooting system...".to_string()));

    // Reboot the system
    match std::process::Command::new("reboot").output() {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e)),
    }
}
