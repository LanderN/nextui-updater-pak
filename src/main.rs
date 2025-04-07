#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(dead_code)]

use bytes::Bytes;
use egui::{Button, Color32, FullOutput, ProgressBar};
use egui_backend::egui;
use egui_backend::{sdl2::event::Event, DpiScaling, ShaderVersion};
use egui_sdl2_gl as egui_backend;
use egui_sdl2_gl::egui::{
    CornerRadius, FontData, FontDefinitions, FontFamily, Pos2, Rect, RichText, Spinner, Vec2,
};
use parking_lot::Mutex;
use reqwest::blocking::RequestBuilder;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::exit;
use std::{
    fs::File,
    io::{Cursor, Read, Write},
    sync::Arc,
    thread,
    time::Instant,
};
use zip::read::root_dir_common_filter;

// Define GitHub API response structures
#[derive(Deserialize, Clone, Debug)]
struct Asset {
    name: String,
    url: String,
}

#[derive(Deserialize, Clone, Debug)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize, Clone, Debug)]
struct Tag {
    pub name: String,
    pub commit: Commit,
}

#[derive(Deserialize, Clone, Debug)]
struct Commit {
    pub sha: String,
}

// Application state shared between UI thread and update thread

enum Progress {
    Indeterminate,
    Determinate(f32),
}

struct AppState {
    submenu: Submenu,
    nextui_release: Option<Release>,
    nextui_tag: Option<Tag>,
    pakman_release: Option<Release>,
    current_operation: Option<String>,
    progress: Option<Progress>,
    error: Option<String>,
    hint: Option<String>,
    should_quit: bool,
}

#[derive(Clone, Copy)]
enum Submenu {
    None,
    NextUI,
    Pakman,
}

// Constants
const USER_AGENT: &str = "NextUI Updater";
const SDCARD_ROOT: &str = "/mnt/SDCARD/";
const WINDOW_WIDTH: u32 = 1024;
const WINDOW_HEIGHT: u32 = 768;
const DPI_SCALE: f32 = 4.0;

const FONTS: [&str; 2] = ["BPreplayBold-unhinted.otf", "chillroundm.ttf"];

// Error type for the application
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn fetch_latest_release(repo: &str) -> Result<Release> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(format!(
            "https://api.github.com/repos/{repo}/releases/latest"
        ))
        .header("User-Agent", USER_AGENT)
        .send()?;

    if !response.status().is_success() {
        return Err(format!("GitHub API request failed: {}", response.status()).into());
    }

    Ok(response.json()?)
}

fn fetch_tag(repo: &str, tag: &str) -> Result<Tag> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(format!("https://api.github.com/repos/{repo}/tags"))
        .header("User-Agent", USER_AGENT)
        .send()?;

    if !response.status().is_success() {
        return Err(format!("GitHub API request failed: {}", response.status()).into());
    }

    let tags: Vec<Tag> = response.json()?;

    let tag = tags.iter().find(|t| t.name == tag).ok_or("Tag not found")?;

    Ok(tag.clone())
}

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

fn download(request: RequestBuilder, progress_cb: impl Fn(f32)) -> Result<Bytes> {
    let mut response = request.send()?;
    let total_size = response.content_length().unwrap_or(0);

    let mut bytes = Vec::new();
    let mut downloaded: u64 = 0;
    let mut buffer = [0; 16384];

    loop {
        let bytes_read = response.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        bytes.write_all(&buffer[..bytes_read])?;
        downloaded += bytes_read as u64;

        // Show progress
        if total_size > 0 {
            let percentage = downloaded as f64 / total_size as f64;
            progress_cb(percentage as f32);
        }
    }

    println!("\nDownload complete!");
    println!("Status: {}", response.status());
    println!("Headers: {:?}", response.headers());

    Ok(bytes.into())
}

fn self_update(app_state: Arc<Mutex<AppState>>) -> Result<()> {
    // Fetch latest release information
    {
        let mut state = app_state.lock();
        state.current_operation = Some("Fetching latest updater release...".to_string());
        state.progress = Some(Progress::Indeterminate);
    }

    println!("Fetching latest updater release...");

    let release = fetch_latest_release("LanderN/nextui-updater-pak")?;

    println!("Latest updater release: {release:?}");

    let available = semver::Version::parse(&release.tag_name)?;
    let installed = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;

    if available > installed {
        println!("New version available: {available} (current: {installed})",);

        let mut state = app_state.lock();
        state.current_operation = Some("Downloading updater...".to_string());
    } else {
        println!("No updates available");

        return Ok(());
    }

    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .timeout(None)
        .build()?;

    let request_builder = client
        .get(&release.assets[0].url)
        .header("Accept", "application/octet-stream")
        .header("User-Agent", USER_AGENT);

    let bytes = download(request_builder, |pr| {
        let mut state = app_state.lock();
        state.progress = Some(Progress::Determinate(pr));
    })?;

    {
        let mut state = app_state.lock();
        state.current_operation =
            format!("Extracting NextUI Updater {}...", release.tag_name).into();
        state.progress = Some(Progress::Indeterminate);
    }

    // Move the current binary to a backup location
    let current_binary = std::env::current_exe()?;
    std::fs::rename(&current_binary, current_binary.with_extension("bak"))?;

    // Extract the update package
    let result = extract_zip(bytes, false, |pr| {
        let mut state = app_state.lock();
        state.progress = Some(Progress::Determinate(pr));
    });

    if result.is_err() {
        // Move the backup back
        std::fs::rename(current_binary.with_extension("bak"), current_binary)?;

        return Err("Failed to extract update package".into());
    }

    {
        let mut state = app_state.lock();
        state.current_operation = Some("Self-update success! Restarting updater...".to_string());
    }

    // Give the user a moment to see the completion message
    thread::sleep(std::time::Duration::from_secs(1));

    // "5" is the exit code for "restart required"
    exit(5);
}

fn do_nextui_release_check(app_state: Arc<Mutex<AppState>>) {
    // Fetch latest release information
    {
        let mut state = app_state.lock();
        state.current_operation = Some("Fetching latest NextUI release...".to_string());
        state.progress = Some(Progress::Indeterminate);
    }

    let latest_release = fetch_latest_release("LoveRetro/NextUI");

    {
        let mut state = app_state.lock();
        match &latest_release {
            Ok(release) => {
                state.nextui_release = Some(release.clone());
            }
            Err(err) => {
                state.error = Some(format!("Fetch failed: {err}"));
                state.current_operation = None;
                state.progress = None;
            }
        }
    }

    if latest_release.is_err() {
        return;
    }
    let latest_release = latest_release.unwrap();

    // Fetch latest tag information
    {
        let mut state = app_state.lock();
        state.current_operation = Some("Fetching latest NextUI tag...".to_string());
        state.progress = Some(Progress::Indeterminate);
    }

    let latest_tag = fetch_tag("LoveRetro/NextUI", &latest_release.tag_name);
    {
        let mut state = app_state.lock();
        match latest_tag {
            Ok(tag) => {
                state.nextui_tag = Some(tag.clone());
            }
            Err(err) => {
                state.error = Some(format!("Fetch failed: {err}"));
            }
        }
    }

    {
        let mut state = app_state.lock();
        state.current_operation = None;
        state.progress = None;
    }
}

fn do_pakman_release_check(app_state: Arc<Mutex<AppState>>) {
    thread::spawn(move || {
        // Fetch latest release information
        {
            let mut state = app_state.lock();
            state.current_operation = Some("Fetching latest Pakman release...".to_string());
            state.progress = Some(Progress::Indeterminate);
        }

        match fetch_latest_release("josegonzalez/pakman") {
            Ok(release) => {
                let mut state = app_state.lock();
                state.pakman_release = Some(release.clone());
                state.current_operation = None;
                state.progress = None;
            }
            Err(err) => {
                let mut state = app_state.lock();
                state.current_operation = None;
                state.error = Some(format!("Fetch failed: {err}"));
                state.progress = None;
            }
        }
    });
}

fn do_self_update(app_state: Arc<Mutex<AppState>>) {
    // Do self-update
    let result = { self_update(app_state.clone()) };
    match result {
        Ok(()) => {
            let mut state = app_state.lock();
            state.current_operation = None;
            state.progress = None;
        }
        Err(err) => {
            let mut state = app_state.lock();
            state.current_operation = None;
            state.error = Some(format!("Self-update failed: {err}"));
            state.progress = None;
        }
    }
}

fn do_update(app_state: Arc<Mutex<AppState>>, full: bool) {
    thread::spawn(move || {
        if let Err(err) = update_nextui(app_state.clone(), full) {
            let mut state = app_state.lock();
            state.current_operation = None;
            state.error = Some(format!("Update failed: {err}"));
            state.progress = None;
            drop(state);

            // Try to fetch latest release information again
            do_nextui_release_check(app_state.clone());
        }
    });
}

fn update_nextui(app_state: Arc<Mutex<AppState>>, full: bool) -> Result<()> {
    let release = {
        let mut state = app_state.lock();
        let release = state.nextui_release.clone().ok_or("No release found")?;

        state.current_operation = Some("Downloading update...".to_string());
        state.progress = Some(Progress::Indeterminate);

        release
    };

    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .timeout(None)
        .build()?;

    let assets = release.assets;
    let asset = assets
        .iter()
        .find(|a| a.name.contains(if full { "all" } else { "base" }))
        .or(assets.first())
        .ok_or("No assets found")?;
    // Download the asset
    {
        let mut state = app_state.lock();
        state.current_operation = format!("Downloading {}...", asset.name).into();
        state.progress = Some(Progress::Indeterminate);
    }

    println!("Downloading from {}", asset.url);

    let request_builder = client
        .get(&asset.url)
        .header("Accept", "application/octet-stream")
        .header("User-Agent", USER_AGENT);

    let bytes = download(request_builder, |pr| {
        let mut state = app_state.lock();
        state.progress = Some(Progress::Determinate(pr));
    })?;

    {
        let mut state = app_state.lock();
        state.current_operation = format!("Extracting {}...\nPlease wait...", asset.name).into();
        state.progress = Some(Progress::Indeterminate);
    }

    // Extract the update package

    if full {
        // Full update, extract all files
        extract_zip(bytes, false, |pr| {
            let mut state = app_state.lock();
            state.progress = Some(Progress::Determinate(pr));
        })?;
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

    {
        let mut state = app_state.lock();
        state.current_operation = Some("Update complete, preparing to reboot...".to_string());
    }

    // Give the user a moment to see the completion message
    thread::sleep(std::time::Duration::from_secs(2));

    {
        let mut state = app_state.lock();
        state.current_operation = Some("Rebooting system...".to_string());
    }

    // Reboot the system
    match std::process::Command::new("reboot").output() {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e)),
    }
}

fn do_pakman_update(app_state: Arc<Mutex<AppState>>) {
    thread::spawn(move || {
        if let Err(err) = update_pakman(app_state.clone()) {
            let mut state = app_state.lock();
            state.current_operation = None;
            state.error = Some(format!("Update failed: {err}"));
            state.progress = None;
        }
    });
}

fn update_pakman(app_state: Arc<Mutex<AppState>>) -> Result<()> {
    let release = {
        let state = app_state.lock();
        state.pakman_release.clone().ok_or("No release found")?
    };

    let client = reqwest::blocking::Client::new();

    {
        let mut state = app_state.lock();
        state.current_operation = Some(format!("Downloading Pakman {}...", release.tag_name));
        state.progress = Some(Progress::Indeterminate);
    }

    println!("Downloading from {}", release.assets[0].url);

    let request_builder = client
        .get(&release.assets[0].url)
        .header("Accept", "application/octet-stream")
        .header("User-Agent", USER_AGENT);

    let bytes = download(request_builder, |pr| {
        let mut state = app_state.lock();
        state.progress = Some(Progress::Determinate(pr));
    })?;

    {
        let mut state = app_state.lock();
        state.current_operation = format!(
            "Extracting Pakman {}...\nPlease be patient, this may take a while...",
            release.tag_name
        )
        .into();
        state.progress = Some(Progress::Indeterminate);
    }

    extract_zip(bytes, true, |pr| {
        let mut state = app_state.lock();
        state.progress = Some(Progress::Determinate(pr));
    })?;

    {
        let mut state = app_state.lock();
        state.current_operation = Some("Pakman update success!".to_string());
    }

    // wait a bit
    thread::sleep(std::time::Duration::from_secs(4));

    {
        let mut state = app_state.lock();
        state.current_operation = None;
        state.progress = None;
    }

    Ok(())
}

// Map controller buttons to keyboard keys
fn controller_to_key(button: sdl2::controller::Button) -> Option<sdl2::keyboard::Keycode> {
    match button {
        sdl2::controller::Button::DPadUp => Some(sdl2::keyboard::Keycode::Up),
        sdl2::controller::Button::DPadDown => Some(sdl2::keyboard::Keycode::Down),
        sdl2::controller::Button::DPadLeft => Some(sdl2::keyboard::Keycode::Left),
        sdl2::controller::Button::DPadRight => Some(sdl2::keyboard::Keycode::Right),
        sdl2::controller::Button::B => Some(sdl2::keyboard::Keycode::Return),
        sdl2::controller::Button::A => Some(sdl2::keyboard::Keycode::Escape),
        _ => None,
    }
}

fn setup_ui_style() -> egui::Style {
    let mut style = egui::Style::default();
    style.spacing.button_padding = Vec2::new(8.0, 2.0);

    style.visuals.panel_fill = Color32::from_rgb(0, 0, 0);
    style.visuals.selection.bg_fill = Color32::WHITE;
    style.visuals.selection.stroke.color = Color32::GRAY;

    style.visuals.widgets.inactive.fg_stroke.color = Color32::WHITE;
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;

    style.visuals.widgets.active.bg_fill = Color32::WHITE;
    style.visuals.widgets.active.weak_bg_fill = Color32::WHITE;
    style.visuals.widgets.active.fg_stroke.color = Color32::BLACK;
    style.visuals.widgets.active.corner_radius = CornerRadius::same(255);

    style.visuals.widgets.noninteractive.fg_stroke.color = Color32::WHITE;
    style.visuals.widgets.noninteractive.bg_fill = Color32::TRANSPARENT;

    style.visuals.widgets.hovered.bg_fill = Color32::WHITE;
    style.visuals.widgets.hovered.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(255);

    style
}

fn init_sdl() -> Result<(
    sdl2::Sdl,
    sdl2::video::Window,
    sdl2::EventPump,
    Option<sdl2::controller::GameController>,
)> {
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;

    // Initialize game controller subsystem
    let game_controller_subsystem = sdl_context.game_controller()?;
    let available = game_controller_subsystem.num_joysticks()?;

    // Attempt to open the first available game controller
    let controller = (0..available).find_map(|id| {
        if !game_controller_subsystem.is_game_controller(id) {
            return None;
        }

        match game_controller_subsystem.open(id) {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("Failed to open controller {id}: {e:?}");
                None
            }
        }
    });

    // Create a window
    let window = video_subsystem
        .window(
            &format!("NextUI Updater {}", env!("CARGO_PKG_VERSION")),
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
        )
        .position_centered()
        .opengl()
        .build()?;

    let event_pump = sdl_context.event_pump()?;

    Ok((sdl_context, window, event_pump, controller))
}

fn nextui_ui(
    ui: &mut egui::Ui,
    app_state: Arc<Mutex<AppState>>,
    current_version: Option<&str>,
) -> egui::Response {
    let latest_release = app_state.lock().nextui_release.clone();
    let latest_tag = app_state.lock().nextui_tag.clone();
    let mut update_available = true;

    // Show release information if available
    match (current_version, latest_tag, latest_release) {
        (Some(current_version), Some(tag), _) => {
            if tag.commit.sha.starts_with(current_version) {
                ui.label(
                    RichText::new(format!(
                        "You currently have the latest available version:\nNextUI {}",
                        tag.name
                    ))
                    .size(10.0),
                );
                update_available = false;
            } else {
                ui.label(
                    RichText::new(format!("New version available: NextUI {}", tag.name)).size(10.0),
                );
            }
        }
        (_, _, Some(release)) => {
            let version = format!("Latest version: NextUI {}", release.tag_name);
            ui.label(RichText::new(version).size(10.0));
        }
        _ => {
            ui.label(RichText::new("No release information available".to_string()).size(10.0));
        }
    }

    ui.add_space(8.0);

    if update_available {
        let quick_update_button = ui.add(Button::new("Quick Update"));

        // Initiate update if button clicked
        if quick_update_button.clicked() {
            // Clear any previous errors
            app_state.lock().error = None;
            do_update(app_state.clone(), false);
        }

        ui.add_space(4.0);

        let full_update_button = ui.add(Button::new("Full Update"));

        if full_update_button.clicked() {
            // Clear any previous errors
            app_state.lock().error = None;
            do_update(app_state.clone(), true);
        }

        // HINTS
        if quick_update_button.has_focus() {
            app_state.lock().hint = Some("Update MinUI.zip only".to_string());
        } else if full_update_button.has_focus() {
            app_state.lock().hint = Some("Extract full zip files (base + extras)".to_string());
        } else {
            app_state.lock().hint = None;
        }

        quick_update_button
    } else {
        let button = ui.button("Quit");

        if button.clicked() {
            app_state.lock().should_quit = true;
        }

        button
    }
}

fn pakman_ui(ui: &mut egui::Ui, app_state: Arc<Mutex<AppState>>) -> egui::Response {
    let latest_release = app_state.lock().pakman_release.clone();

    // Show release information if available
    if let Some(release) = latest_release {
        let version = format!("Latest version: Pakman {}", release.tag_name);
        ui.label(RichText::new(version).size(10.0));
    }

    ui.add_space(8.0);

    let button = ui.button("Update Pakman");
    if button.clicked() {
        // Clear any previous errors
        app_state.lock().error = None;
        do_pakman_update(app_state.clone());
    }

    // HINTS
    if button.has_focus() {
        app_state.lock().hint = Some("Update pakman by josegonzalez (aka savant)".to_string());
    } else {
        app_state.lock().hint = None;
    }

    button
}

// Load font from file
fn load_font() -> Result<FontDefinitions> {
    fn get_font_preference() -> Result<usize> {
        // Load NextUI settings
        let mut settings_file =
            std::fs::File::open(SDCARD_ROOT.to_owned() + ".userdata/shared/minuisettings.txt")?;

        let mut settings = String::new();
        settings_file.read_to_string(&mut settings)?;

        println!("Settings: {settings}");

        // Very crappy parser
        Ok(settings.contains("font=1").into())
    }

    // Now load the font
    let mut path = PathBuf::from(SDCARD_ROOT);
    path.push(format!(
        ".system/res/{}",
        FONTS[get_font_preference().unwrap_or(0)]
    ));
    println!("Loading font: {}", path.display());
    let mut font_bytes = vec![];
    std::fs::File::open(path)?.read_to_end(&mut font_bytes)?;

    let mut font_data: BTreeMap<String, Arc<FontData>> = BTreeMap::new();

    let mut families = BTreeMap::new();

    font_data.insert(
        "custom_font".to_owned(),
        std::sync::Arc::new(FontData::from_owned(font_bytes)),
    );

    families.insert(FontFamily::Proportional, vec!["custom_font".to_owned()]);
    families.insert(FontFamily::Monospace, vec!["custom_font".to_owned()]);

    Ok(FontDefinitions {
        font_data,
        families,
    })
}

#[allow(clippy::too_many_lines)]
fn main() -> Result<()> {
    // Initialize SDL and create window
    let (_sdl_context, window, mut event_pump, _controller) = init_sdl()?;

    // Create OpenGL context and egui painter
    let _gl_context = window.gl_create_context()?;
    let shader_ver = ShaderVersion::Adaptive;
    let (mut painter, mut egui_state) =
        egui_backend::with_sdl2(&window, shader_ver, DpiScaling::Custom(DPI_SCALE));

    // Create egui context and set style
    let egui_ctx = egui::Context::default();
    egui_ctx.set_style(setup_ui_style());

    // Font stuff

    if let Ok(fonts) = load_font() {
        egui_ctx.set_fonts(fonts);
    }

    // Initialize application state
    let app_state = Arc::new(Mutex::new(AppState {
        submenu: Submenu::NextUI,
        nextui_release: None,
        nextui_tag: None,
        pakman_release: None,
        current_operation: None,
        progress: None,
        error: None,
        hint: None,
        should_quit: false,
    }));

    let enter_submenu = |s: Submenu| {
        let mut state = app_state.lock();
        state.submenu = s;
        state.hint = None;
    };

    // Get current NextUI version
    let version_file = std::fs::read_to_string(SDCARD_ROOT.to_owned() + ".system/version.txt")?;
    let current_sha = version_file.lines().nth(1);

    // Self-update
    let app_state_clone = app_state.clone();
    thread::spawn(move || {
        do_self_update(app_state_clone.clone());
        do_nextui_release_check(app_state_clone.clone());
    });

    let start_time: Instant = Instant::now();

    'running: loop {
        if app_state.lock().should_quit {
            break;
        }

        egui_state.input.time = Some(start_time.elapsed().as_secs_f64());
        egui_ctx.begin_pass(egui_state.input.take());

        // UI rendering
        egui::CentralPanel::default().show(&egui_ctx, |ui| {
            ui.vertical_centered(|ui| {
                // Check application state
                let state_lock = app_state.lock();
                let update_in_progress = state_lock.current_operation.is_some();
                drop(state_lock);

                ui.label(
                    RichText::new(format!("NextUI Updater {}", env!("CARGO_PKG_VERSION")))
                        .color(Color32::from_rgb(150, 150, 150))
                        .size(10.0),
                );
                ui.add_space(4.0);

                ui.add_enabled_ui(!update_in_progress, |ui| {
                    let submenu =  { app_state.lock().submenu };
                    let menu = match submenu {
                        Submenu::None => {
                            let nextui_button = ui.button("NextUI");
                            if nextui_button.clicked() {
                                do_nextui_release_check(app_state.clone());
                                enter_submenu(Submenu::NextUI);
                            }
                            let pakman_button = ui.button("Pakman");
                            if pakman_button.clicked() {
                                do_pakman_release_check(app_state.clone());
                                enter_submenu(Submenu::Pakman);
                            }
                            ui.add_space(4.0);
                            if ui.button("Quit").clicked() {
                                app_state.lock().should_quit = true;
                            }

                            // HINTS
                            if nextui_button.has_focus() {
                                app_state.lock().hint =
                                    Some("Update NextUI".to_string());
                            } else if pakman_button.has_focus() {
                                app_state.lock().hint =
                                    Some("Update pakman by josegonzalez (aka savant)".to_string());
                            } else {
                                app_state.lock().hint = None;
                            }

                            nextui_button
                        }
                        Submenu::NextUI => nextui_ui(ui, app_state.clone(), current_sha),
                        Submenu::Pakman => pakman_ui(ui, app_state.clone()),
                    };

                    // Focus the first available button for controller navigation
                    ui.memory_mut(|r| {
                        if r.focused().is_none() {
                            r.request_focus(menu.id);
                        }
                    });
                });

                ui.add_space(8.0);

                // Display current operation
                if let Some(operation) = &app_state.lock().current_operation {
                    ui.label(RichText::new(operation).color(Color32::from_rgb(150, 150, 150)).size(10.0));
                }

                // Display error if any
                if let Some(error) = &app_state.lock().error {
                    ui.colored_label(Color32::from_rgb(255, 150, 150), RichText::new(error));
                }

                // Show progress bar if available
                if let Some(progress) = &app_state.lock().progress {
                    match progress {
                        Progress::Indeterminate => {
                            ui.add_space(4.0);
                            ui.add(Spinner::new().color(Color32::WHITE));
                        }
                        Progress::Determinate(pr) => {
                            ui.add(ProgressBar::new(*pr).show_percentage());
                        }
                    }
                }
            });

            if let Some(hint) = &app_state.lock().hint {
                ui.allocate_new_ui(
                    egui::UiBuilder::new().max_rect(Rect {
                        min: Pos2 {
                            x: 0.0,
                            y: ui.max_rect().height() - 2.0,
                        },
                        max: Pos2 {
                            x: 1024.0 / DPI_SCALE,
                            y: ui.max_rect().height(),
                        },
                    }),
                    |ui| {
                        ui.centered_and_justified(|ui| {
                            ui.label(RichText::new(hint).size(10.0));
                        });
                    },
                );
            }

            // HACK: for some reason dynamic text isn't rendered without this
            ui.allocate_ui(
                Vec2::ZERO,
                |ui| {
                    ui.label(
                        RichText::new(
                            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789~`!@#$%^&*()-=_+[]{};':\",.<>/?",
                        )
                        .size(10.0)
                        .color(Color32::TRANSPARENT)
                    );
                },
            );
        });

        // End frame and render
        let FullOutput {
            platform_output,
            textures_delta,
            shapes,
            pixels_per_point,
            viewport_output: _,
        } = egui_ctx.end_pass();

        // Process output
        egui_state.process_output(&window, &platform_output);

        // Paint and swap buffers
        let paint_jobs = egui_ctx.tessellate(shapes, pixels_per_point);
        painter.paint_jobs(None, textures_delta, paint_jobs);
        window.gl_swap_window();

        let handle_back_button = || {
            // for now we always quit
            app_state.lock().should_quit = true;

            // let submenu = { app_state.lock().submenu };
            // match submenu {
            //     Submenu::None => {
            //         app_state.lock().should_quit = true;
            //     }
            //     Submenu::NextUI => {
            //         enter_submenu(Submenu::None);
            //     }
            //     Submenu::Pakman => {
            //         enter_submenu(Submenu::None);
            //     }
            // }
        };

        // Process events
        if let Some(event) = event_pump.wait_event_timeout(5) {
            match event {
                Event::Quit { .. } => break 'running,
                Event::ControllerButtonDown {
                    timestamp, button, ..
                } => {
                    if let Some(keycode) = controller_to_key(button) {
                        let key_event = Event::KeyDown {
                            keycode: Some(keycode),
                            timestamp,
                            window_id: window.id(),
                            scancode: Some(sdl2::keyboard::Scancode::Down),
                            keymod: sdl2::keyboard::Mod::empty(),
                            repeat: false,
                        };
                        egui_state.process_input(&window, key_event, &mut painter);
                    }
                }
                Event::ControllerButtonUp {
                    timestamp, button, ..
                } => {
                    if button == sdl2::controller::Button::A {
                        // Exit with "B" button
                        handle_back_button();
                    }

                    if let Some(keycode) = controller_to_key(button) {
                        let key_event = Event::KeyUp {
                            keycode: Some(keycode),
                            timestamp,
                            window_id: window.id(),
                            scancode: Some(sdl2::keyboard::Scancode::Down),
                            keymod: sdl2::keyboard::Mod::empty(),
                            repeat: false,
                        };

                        egui_state.process_input(&window, key_event, &mut painter);
                    }
                }
                // for easy testing on desktop
                Event::KeyDown {
                    keycode: Some(sdl2::keyboard::Keycode::Escape),
                    ..
                } => {
                    handle_back_button();
                }
                _ => {
                    // Process other input events
                    egui_state.process_input(&window, event, &mut painter);
                }
            }
        }
    }

    Ok(())
}
