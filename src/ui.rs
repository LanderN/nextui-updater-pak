use crate::app_state::{AppStateManager, Progress, Submenu};
use crate::update::do_update;
use egui::{Button, Color32, FullOutput, ProgressBar};
use egui_backend::egui;
use egui_backend::{sdl2::event::Event, DpiScaling, ShaderVersion};
use egui_sdl2_gl as egui_backend;
use egui_sdl2_gl::egui::{
    CornerRadius, FontData, FontDefinitions, FontFamily, Pos2, Rect, RichText, Spinner, Vec2,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::{io::Read, sync::Arc, time::Instant};

use crate::{Result, SDCARD_ROOT};

const WINDOW_WIDTH: u32 = 1024;
const WINDOW_HEIGHT: u32 = 768;
const DPI_SCALE: f32 = 4.0;
const FONTS: [&str; 2] = ["BPreplayBold-unhinted.otf", "chillroundm.ttf"];

fn nextui_ui(ui: &mut egui::Ui, app_state: &'static AppStateManager) -> egui::Response {
    let current_version = app_state.current_version();
    let mut latest_release = app_state.nextui_release().clone();
    let mut latest_tag = app_state.nextui_tag().clone();
    let mut update_available = true;
    let latest_discarded = app_state.nextui_tag().clone().is_none();

    if app_state.release_selection_menu() {
        let index = app_state.nextui_releases_and_tags_index().unwrap_or(0);
        let relase_and_tag_vector = app_state.nextui_releases_and_tags().unwrap_or_default();
        latest_release = Some(relase_and_tag_vector[index].release.clone());
        latest_tag = Some(relase_and_tag_vector[index].tag.clone());
    }

    if app_state.release_selection_menu() & !app_state.release_selection_confirmed() {
        ui.add_space(16.0);
        ui.label(
            RichText::new(
                "WARNING\n\
            Downgrades are not fully supported by NextUI!\n\
            Some settings may be lost or unstable in old versions\n\
            Manual editing of settings or files may be required",
            )
            .size(10.0),
        );
    } else {
        // Show release information if available
        match (current_version, latest_tag, latest_release) {
            (Some(current_version), Some(tag), _) => {
                let selected_tag = hint_wrap_nextui_tag(app_state, tag.name);
                if tag.commit.sha.starts_with(&current_version) & !latest_discarded {
                    if app_state.release_selection_menu() {
                        // selection view
                        ui.label(
                            RichText::new(format!("Selected Version:\n{}\nThis version is currently already installed!", selected_tag)).size(10.0),
                        );
                    } else {
                        ui.label(
                            RichText::new(format!("You currently have the latest available version:\n{}\nX to select different version", selected_tag)).size(10.0),
                        );
                    }
                    update_available = false;
                } else {
                    if app_state.release_selection_menu() {
                        // selection view
                        ui.label(
                            RichText::new(format!("Selected Version:\n{}", selected_tag))
                                .size(10.0),
                        );
                    } else {
                        ui.label(
                            RichText::new(format!(
                                "New version available:\n{}\nX to select different version",
                                selected_tag
                            ))
                            .size(10.0),
                        );
                    }
                }
            }
            (_, _, Some(release)) => {
                if app_state.release_selection_menu() {
                    // selection view
                    let selected_tag = hint_wrap_nextui_tag(app_state, release.tag_name);
                    ui.label(
                        RichText::new(format!("Selected Version:\n{}", selected_tag)).size(10.0),
                    );
                } else {
                    ui.label(
                        RichText::new(format!(
                            "Latest version:\nNextUI {}\nX to select different version",
                            release.tag_name
                        ))
                        .size(10.0),
                    );
                }
            }
            _ => {
                ui.label(RichText::new("No release information available".to_string()).size(10.0));
            }
        }
    }

    ui.add_space(8.0);

    if app_state.release_selection_menu() & !app_state.release_selection_confirmed() {
        let back_button = ui.button("Return");
        if back_button.clicked() {
            app_state.set_release_selection_menu(false);
        }

        let confirm_button = ui.button("Accept Warning");
        if confirm_button.clicked() {
            app_state.set_release_selection_confirmed(true);
        }

        if back_button.has_focus() {
            app_state.set_hint(Some("Return to Latest Version options".to_string()));
        } else if confirm_button.has_focus() {
            app_state.set_hint(Some("Confirm warning and open update options".to_string()));
        } else {
            app_state.set_hint(None);
        }

        back_button
    } else if update_available {
        let quick_update_button = ui.add(Button::new("Quick Update"));

        // Initiate update if button clicked
        if quick_update_button.clicked() {
            // Clear any previous errors
            app_state.set_error(None);
            do_update(app_state, false);
        }

        ui.add_space(4.0);

        let full_update_button = ui.add(Button::new("Full Update"));

        if full_update_button.clicked() {
            // Clear any previous errors
            app_state.set_error(None);
            do_update(app_state, true);
        }

        // HINTS
        if quick_update_button.has_focus() {
            app_state.set_hint(Some("Update MinUI.zip only".to_string()));
        } else if full_update_button.has_focus() {
            app_state.set_hint(Some("Extract full zip files (base + extras)".to_string()));
        } else {
            app_state.set_hint(None);
        }

        quick_update_button
    } else {
        let force_button = ui.button("Update anyway");
        if force_button.clicked() {
            app_state.set_nextui_tag(None); // forget the tag
        }

        let quit_button = ui.button("Quit");
        if quit_button.clicked() {
            if app_state.release_selection_menu() {
                app_state.set_release_selection_menu(false);
            } else {
                app_state.set_should_quit(true);
            }
        }

        if quit_button.has_focus() {
            if app_state.release_selection_menu() {
                app_state.set_hint(Some("Return to Latest Version options".to_string()));
            } else {
                app_state.set_hint(Some("Quit NextUI Updater".to_string()));
            }
        } else if force_button.has_focus() {
            app_state.set_hint(Some("Ignore current version".to_string()));
        } else {
            app_state.set_hint(None);
        }

        quit_button
    }
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
        sdl2::controller::Button::Y => Some(sdl2::keyboard::Keycode::X),
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

// Load font from file
fn load_font() -> Result<FontDefinitions> {
    fn get_font_preference() -> Result<usize> {
        // Load NextUI settings
        let mut settings_file =
            std::fs::File::open(SDCARD_ROOT.to_owned() + ".userdata/shared/minuisettings.txt")?;

        let mut settings = String::new();
        settings_file.read_to_string(&mut settings)?;

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

fn hint_wrap_nextui_tag(app_state: &'static AppStateManager, tag_name: String) -> String {
    let mut selected_tag = format!("NextUI {}", tag_name);
    if !app_state.release_selection_menu() {
        return selected_tag;
    }
    if !is_most_left_index(app_state) {
        selected_tag = format!("<<     {}", selected_tag);
    }
    if !is_most_right_index(app_state) {
        selected_tag = format!("{}     >>", selected_tag);
    }
    return selected_tag;
}

fn is_most_left_index(app_state: &'static AppStateManager) -> bool {
    let index = app_state.nextui_releases_and_tags_index().unwrap_or(0);
    let max_index = app_state
        .nextui_releases_and_tags()
        .unwrap_or_default()
        .len();
    return index >= max_index - 1;
}

fn is_most_right_index(app_state: &'static AppStateManager) -> bool {
    return app_state.nextui_releases_and_tags_index() == Some(0);
}

#[allow(clippy::too_many_lines)]
pub fn run_ui(app_state: &'static AppStateManager) -> Result<()> {
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

    let start_time: Instant = Instant::now();

    loop {
        if app_state.should_quit() {
            break;
        }

        egui_state.input.time = Some(start_time.elapsed().as_secs_f64());
        egui_ctx.begin_pass(egui_state.input.take());

        // UI rendering
        egui::CentralPanel::default().show(&egui_ctx, |ui| {
            ui.vertical_centered(|ui| {
                // Check application state
                let update_in_progress = app_state.current_operation().is_some();

                let title_prefix = format!("NextUI Updater {}", env!("CARGO_PKG_VERSION"));
                if app_state.release_selection_menu() {
                    if app_state.release_selection_confirmed() {
                        ui.label(
                            RichText::new(title_prefix + " Version Selector")
                                .color(Color32::from_rgb(150, 150, 150))
                                .size(10.0),
                        );
                    } else {
                        ui.label(
                            RichText::new(title_prefix + " Version Selector Warning")
                                .color(Color32::from_rgb(150, 150, 150))
                                .size(10.0),
                        );
                    }
                } else {
                    ui.label(
                        RichText::new(title_prefix)
                            .color(Color32::from_rgb(150, 150, 150))
                            .size(10.0),
                    );
                }
                ui.add_space(4.0);

                ui.add_enabled_ui(!update_in_progress, |ui| {
                    let submenu = app_state.submenu();
                    let menu = match submenu {
                        Submenu::NextUI => nextui_ui(ui, app_state),
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
                if let Some(operation) = app_state.current_operation() {
                    ui.label(RichText::new(operation).color(Color32::from_rgb(150, 150, 150)).size(10.0));
                }

                // Display error if any
                if let Some(error) = app_state.error() {
                    ui.colored_label(Color32::from_rgb(255, 150, 150), RichText::new(error));
                }

                // Show progress bar if available
                if let Some(progress) = app_state.progress() {
                    match progress {
                        Progress::Indeterminate => {
                            ui.add_space(4.0);
                            ui.add(Spinner::new().color(Color32::WHITE));
                        }
                        Progress::Determinate(pr) => {
                            let mut progress_bar = ProgressBar::new(pr);
                            // Show percentage only if progress is > 10% to avoid text
                            // escaping the progress bar
                            if pr > 0.1 {
                                progress_bar = progress_bar.show_percentage();
                            }
                            ui.add(progress_bar);
                        }
                    }
                }
            });

            if let Some(hint) = app_state.hint() {
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
            viewport_output,
        } = egui_ctx.end_pass();

        let repaint_after = viewport_output
            .get(&egui::ViewportId::ROOT)
            .expect("Missing ViewportId::ROOT")
            .repaint_delay;

        // Process output
        egui_state.process_output(&window, &platform_output);

        // Paint and swap buffers
        let paint_jobs = egui_ctx.tessellate(shapes, pixels_per_point);
        painter.paint_jobs(None, textures_delta, paint_jobs);
        window.gl_swap_window();

        let handle_back_button = || {
            if app_state.release_selection_menu() {
                app_state.set_release_selection_menu(false);
            } else {
                app_state.set_should_quit(true);
            }
        };

        // Process events
        let mut process_event = |event| {
            match event {
                Event::Quit { .. } => app_state.set_should_quit(true),
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

                    if app_state.release_selection_menu() {
                        if app_state.release_selection_confirmed() {
                            // Add left/right options in selection menu
                            let index = app_state.nextui_releases_and_tags_index().unwrap_or(0);
                            if button == sdl2::controller::Button::DPadLeft {
                                if !is_most_left_index(app_state) {
                                    app_state.set_nextui_releases_and_tags_index(Some(index + 1));
                                }
                            }
                            if button == sdl2::controller::Button::DPadRight {
                                if !is_most_right_index(app_state) {
                                    app_state.set_nextui_releases_and_tags_index(Some(index - 1));
                                }
                            }
                        }
                    } else {
                        // Add X button to reach selection menu
                        if button == sdl2::controller::Button::Y {
                            app_state.set_release_selection_menu(true);
                        }
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
        };

        if repaint_after.is_zero() {
            for event in event_pump.poll_iter() {
                process_event(event);
            }
        } else if let Some(event) = event_pump.wait_event_timeout(50) {
            process_event(event);
        }
    }

    Ok(())
}
