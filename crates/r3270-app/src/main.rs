use std::fs;
use std::path::PathBuf;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use r3270_core::config::{ColorSpec, HostProfile, SessionConfig, TerminalModel, ThemeConfig};
use r3270_core::events::{ConnectionState, HostEvent, OiaState};
use r3270_core::presentation::{AidKey, CellAttrs, Color, Highlight, PresentationSpace};
use r3270_filetransfer::{HostKind, IndFileRequest, TransferDirection, TransferMode};
use r3270_net::{SessionCommand, run_session};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "r3270=info,warn".into()),
        )
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("r3270")
            .with_inner_size([1100.0, 760.0]),
        ..Default::default()
    };
    eframe::run_native(
        "r3270",
        options,
        Box::new(|cc| Ok(Box::new(R3270App::new(cc)))),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProfileStore {
    profiles: Vec<HostProfile>,
}

struct R3270App {
    runtime: Runtime,
    profiles: Vec<HostProfile>,
    themes: Vec<ThemeConfig>,
    tabs: Vec<TerminalTab>,
    active_tab: usize,
    next_tab_id: u64,
    profile_editor: HostProfile,
    transfer_request: IndFileRequest,
    transfer_status: String,
}

struct TerminalTab {
    id: u64,
    title: String,
    profile: HostProfile,
    screen: PresentationSpace,
    oia: OiaState,
    command_tx: Option<mpsc::UnboundedSender<SessionCommand>>,
    event_rx: Option<mpsc::UnboundedReceiver<HostEvent>>,
    last_error: Option<String>,
}

impl R3270App {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let profiles = load_profiles().unwrap_or_else(|| vec![HostProfile::default()]);
        let profile_editor = profiles.first().cloned().unwrap_or_default();
        let themes = ThemeConfig::built_ins();
        let first_tab = TerminalTab::new(1, profile_editor.clone());
        Self {
            runtime: Runtime::new().expect("create Tokio runtime"),
            profiles,
            themes,
            tabs: vec![first_tab],
            active_tab: 0,
            next_tab_id: 2,
            profile_editor,
            transfer_request: IndFileRequest::default(),
            transfer_status: String::new(),
        }
    }

    fn active_tab_mut(&mut self) -> Option<&mut TerminalTab> {
        self.tabs.get_mut(self.active_tab)
    }

    fn active_tab(&self) -> Option<&TerminalTab> {
        self.tabs.get(self.active_tab)
    }

    fn poll_events(&mut self) {
        for tab in &mut self.tabs {
            let mut pending = Vec::new();
            if let Some(rx) = tab.event_rx.as_mut() {
                while let Ok(event) = rx.try_recv() {
                    pending.push(event);
                }
            }
            for event in pending {
                match event {
                    HostEvent::Connection(connection) => {
                        tab.oia.connection = connection;
                    }
                    HostEvent::Screen(screen) => {
                        tab.screen = screen;
                    }
                    HostEvent::Oia(oia) => {
                        tab.oia = oia;
                    }
                    HostEvent::ProtocolError(error) => {
                        tab.last_error = Some(error.clone());
                        tab.oia.connection = ConnectionState::Error;
                        tab.oia.message = error;
                    }
                    HostEvent::Disconnected => {
                        tab.command_tx = None;
                        tab.event_rx = None;
                        tab.oia = OiaState::default();
                    }
                    HostEvent::Transfer(progress) => {
                        tab.oia.message = if progress.finished {
                            format!("Transfer finished: {}", progress.label)
                        } else {
                            format!("Transfer running: {}", progress.label)
                        };
                    }
                }
            }
        }
    }

    fn open_tab(&mut self, profile: HostProfile) {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(TerminalTab::new(id, profile));
        self.active_tab = self.tabs.len() - 1;
    }

    fn close_active_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        if let Some(tab) = self.active_tab_mut() {
            if let Some(tx) = &tab.command_tx {
                let _ = tx.send(SessionCommand::Disconnect);
            }
        }
        self.tabs.remove(self.active_tab);
        self.active_tab = self.active_tab.saturating_sub(1).min(self.tabs.len() - 1);
    }

    fn connect_active_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let idx = self.active_tab.min(self.tabs.len() - 1);
        if self.tabs[idx].command_tx.is_some() {
            return;
        }
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let mut config = SessionConfig::default();
        config.profile = self.tabs[idx].profile.clone();
        let event_tx_for_error = event_tx.clone();
        self.runtime.spawn(async move {
            if let Err(err) = run_session(config, command_rx, event_tx).await {
                let _ = event_tx_for_error.send(HostEvent::ProtocolError(err.to_string()));
            }
        });
        let tab = &mut self.tabs[idx];
        tab.command_tx = Some(command_tx);
        tab.event_rx = Some(event_rx);
        tab.oia.connection = ConnectionState::Connecting;
        tab.oia.message = "Connecting".to_owned();
    }

    fn disconnect_active_tab(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            if let Some(tx) = tab.command_tx.take() {
                let _ = tx.send(SessionCommand::Disconnect);
            }
            tab.event_rx = None;
            tab.oia = OiaState::default();
        }
    }

    fn send_input_text(&mut self, text: String) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        if let Some(tx) = &tab.command_tx {
            let _ = tx.send(SessionCommand::InputText(text));
        } else {
            for ch in text.chars() {
                tab.screen.input_char(ch);
            }
        }
    }

    fn send_aid(&mut self, aid: AidKey) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        if let Some(tx) = &tab.command_tx {
            let _ = tx.send(SessionCommand::SendAid(aid));
        } else {
            let _ = tab.screen.aid_response(aid);
            tab.oia.keyboard_locked = tab.screen.keyboard_locked();
        }
    }

    fn send_transfer_command(&mut self) {
        match self.transfer_request.tso_command() {
            Ok(command) => {
                self.send_input_text(command);
                self.send_aid(AidKey::Enter);
                self.transfer_status = "IND$FILE command submitted".to_owned();
            }
            Err(err) => self.transfer_status = err.to_string(),
        }
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let events = ctx.input(|input| input.events.clone());
        for event in events {
            match event {
                egui::Event::Text(text) if !text.is_empty() => self.send_input_text(text),
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => match key {
                    egui::Key::Enter => self.send_aid(AidKey::Enter),
                    egui::Key::Escape => self.send_aid(AidKey::Attn),
                    egui::Key::F1 => self.send_aid(AidKey::Pf(1)),
                    egui::Key::F2 => self.send_aid(AidKey::Pf(2)),
                    egui::Key::F3 => self.send_aid(AidKey::Pf(3)),
                    egui::Key::F4 => self.send_aid(AidKey::Pf(4)),
                    egui::Key::F5 => self.send_aid(AidKey::Pf(5)),
                    egui::Key::F6 => self.send_aid(AidKey::Pf(6)),
                    egui::Key::F7 => self.send_aid(AidKey::Pf(7)),
                    egui::Key::F8 => self.send_aid(AidKey::Pf(8)),
                    egui::Key::F9 => self.send_aid(AidKey::Pf(9)),
                    egui::Key::F10 => self.send_aid(AidKey::Pf(10)),
                    egui::Key::F11 => self.send_aid(AidKey::Pf(11)),
                    egui::Key::F12 => self.send_aid(AidKey::Pf(12)),
                    egui::Key::L if modifiers.ctrl => self.send_aid(AidKey::Clear),
                    egui::Key::S if modifiers.ctrl => self.send_aid(AidKey::SysReq),
                    egui::Key::Backspace => {
                        if let Some(tab) = self.active_tab_mut() {
                            tab.screen.backspace();
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    fn top_bar(&mut self, root: &mut egui::Ui) {
        egui::Panel::top("tabs").show_inside(root, |ui| {
            ui.horizontal(|ui| {
                for idx in 0..self.tabs.len() {
                    let selected = idx == self.active_tab;
                    let response = ui
                        .selectable_label(selected, &self.tabs[idx].title)
                        .on_hover_text(format!("Tab {}", self.tabs[idx].id));
                    if response.clicked() {
                        self.active_tab = idx;
                    }
                }
                if ui.button("+").on_hover_text("New tab").clicked() {
                    self.open_tab(self.profile_editor.clone());
                }
                if ui.button("x").on_hover_text("Close tab").clicked() {
                    self.close_active_tab();
                }
            });
        });
    }

    fn side_panel(&mut self, root: &mut egui::Ui) {
        egui::Panel::left("connection")
            .resizable(false)
            .default_size(280.0)
            .show_inside(root, |ui| {
                ui.heading("Connection");
                ui.separator();
                ui.label("Profile");
                egui::ComboBox::from_id_salt("profiles")
                    .selected_text(&self.profile_editor.name)
                    .show_ui(ui, |ui| {
                        for profile in &self.profiles {
                            if ui
                                .selectable_label(
                                    profile.name == self.profile_editor.name,
                                    &profile.name,
                                )
                                .clicked()
                            {
                                self.profile_editor = profile.clone();
                            }
                        }
                    });
                ui.text_edit_singleline(&mut self.profile_editor.name);
                ui.horizontal(|ui| {
                    ui.label("Host");
                    ui.text_edit_singleline(&mut self.profile_editor.host);
                });
                ui.horizontal(|ui| {
                    ui.label("Port");
                    ui.add(egui::DragValue::new(&mut self.profile_editor.port).range(1..=65535));
                });
                ui.checkbox(&mut self.profile_editor.tls, "TLS");
                ui.checkbox(
                    &mut self.profile_editor.insecure_skip_verify,
                    "Allow insecure TLS",
                );
                ui.horizontal(|ui| {
                    ui.label("LU");
                    let lu = self.profile_editor.lu_name.get_or_insert_with(String::new);
                    ui.text_edit_singleline(lu);
                    if lu.trim().is_empty() {
                        self.profile_editor.lu_name = None;
                    }
                });
                ui.label("Model");
                ui.horizontal_wrapped(|ui| {
                    for (label, model) in [
                        ("2", TerminalModel::Model2),
                        ("3", TerminalModel::Model3),
                        ("4", TerminalModel::Model4),
                        ("5", TerminalModel::Model5),
                    ] {
                        if ui
                            .selectable_label(self.profile_editor.terminal_model == model, label)
                            .clicked()
                        {
                            self.profile_editor.terminal_model = model;
                        }
                    }
                });
                ui.label("Theme");
                egui::ComboBox::from_id_salt("theme")
                    .selected_text(&self.profile_editor.theme)
                    .show_ui(ui, |ui| {
                        for theme in &self.themes {
                            ui.selectable_value(
                                &mut self.profile_editor.theme,
                                theme.name.clone(),
                                &theme.name,
                            );
                        }
                    });

                ui.horizontal(|ui| {
                    if ui.button("Apply to tab").clicked() {
                        let profile = self.profile_editor.clone();
                        if let Some(tab) = self.active_tab_mut() {
                            tab.profile = profile;
                            tab.title = tab.profile.name.clone();
                            tab.screen = PresentationSpace::new(
                                tab.profile.terminal_model,
                                tab.profile.codepage,
                            );
                        }
                    }
                    if ui.button("Save").clicked() {
                        upsert_profile(&mut self.profiles, self.profile_editor.clone());
                        let _ = save_profiles(&self.profiles);
                    }
                });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Connect").clicked() {
                        let profile = self.profile_editor.clone();
                        if let Some(tab) = self.active_tab_mut() {
                            tab.profile = profile;
                            tab.title = tab.profile.name.clone();
                        }
                        self.connect_active_tab();
                    }
                    if ui.button("Disconnect").clicked() {
                        self.disconnect_active_tab();
                    }
                });

                ui.separator();
                ui.heading("File Transfer");
                ui.text_edit_singleline(&mut self.transfer_request.local_file);
                ui.text_edit_singleline(&mut self.transfer_request.host_file);
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.transfer_request.direction,
                        TransferDirection::ReceiveFromHost,
                        "Receive",
                    );
                    ui.selectable_value(
                        &mut self.transfer_request.direction,
                        TransferDirection::SendToHost,
                        "Send",
                    );
                });
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.transfer_request.mode,
                        TransferMode::Text,
                        "Text",
                    );
                    ui.selectable_value(
                        &mut self.transfer_request.mode,
                        TransferMode::Binary,
                        "Binary",
                    );
                });
                self.transfer_request.host_kind = HostKind::Tso;
                ui.checkbox(&mut self.transfer_request.options.append, "Append");
                ui.checkbox(&mut self.transfer_request.options.crlf, "CRLF");
                if ui.button("Submit IND$FILE").clicked() {
                    self.send_transfer_command();
                }
                if !self.transfer_status.is_empty() {
                    ui.label(&self.transfer_status);
                }
            });
    }

    fn bottom_oia(&mut self, root: &mut egui::Ui) {
        egui::Panel::bottom("oia").show_inside(root, |ui| {
            ui.horizontal(|ui| {
                if let Some(tab) = self.active_tab() {
                    let cursor = tab.screen.cursor();
                    ui.monospace(format!("{:?}", tab.oia.connection));
                    ui.separator();
                    ui.monospace(if tab.oia.keyboard_locked {
                        "LOCK"
                    } else {
                        "READY"
                    });
                    ui.separator();
                    ui.monospace(format!("{}x{}", tab.screen.rows(), tab.screen.cols()));
                    ui.separator();
                    ui.monospace(format!("R{} C{}", cursor.row + 1, cursor.col + 1));
                    ui.separator();
                    ui.label(&tab.oia.message);
                    if let Some(error) = &tab.last_error {
                        ui.separator();
                        ui.colored_label(Color32::LIGHT_RED, error);
                    }
                }
            });
        });
    }

    fn central_terminal(&mut self, root: &mut egui::Ui) {
        egui::CentralPanel::default().show_inside(root, |ui| {
            let theme_name = self
                .active_tab()
                .map(|tab| tab.profile.theme.as_str())
                .unwrap_or("green");
            let theme = self
                .themes
                .iter()
                .find(|theme| theme.name == theme_name)
                .unwrap_or(&self.themes[0])
                .clone();
            if let Some(tab) = self.active_tab_mut() {
                render_terminal(ui, tab, &theme);
            }
        });
    }
}

impl eframe::App for R3270App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_events();
        self.handle_keyboard(&ctx);
        self.top_bar(ui);
        self.side_panel(ui);
        self.bottom_oia(ui);
        self.central_terminal(ui);
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}

impl TerminalTab {
    fn new(id: u64, profile: HostProfile) -> Self {
        let title = profile.name.clone();
        let screen = PresentationSpace::new(profile.terminal_model, profile.codepage);
        Self {
            id,
            title,
            profile,
            screen,
            oia: OiaState::default(),
            command_tx: None,
            event_rx: None,
            last_error: None,
        }
    }
}

fn render_terminal(ui: &mut egui::Ui, tab: &mut TerminalTab, theme: &ThemeConfig) {
    let cell_w = 9.6_f32;
    let cell_h = 18.0_f32;
    let cols = tab.screen.cols() as usize;
    let rows = tab.screen.rows() as usize;
    let desired = Vec2::new(cols as f32 * cell_w, rows as f32 * cell_h);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let col = ((pos.x - rect.left()) / cell_w).floor().max(0.0) as u16;
            let row = ((pos.y - rect.top()) / cell_h).floor().max(0.0) as u16;
            if let Some(tx) = &tab.command_tx {
                let _ = tx.send(SessionCommand::MoveCursor { row, col });
            } else {
                tab.screen.set_cursor(row, col);
            }
        }
    }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, color32(&theme.background));
    let font = FontId::monospace(16.0);
    for (idx, cell) in tab.screen.cells().iter().enumerate() {
        let row = idx / cols;
        let col = idx % cols;
        let min = Pos2::new(
            rect.left() + col as f32 * cell_w,
            rect.top() + row as f32 * cell_h,
        );
        let cell_rect = Rect::from_min_size(min, Vec2::new(cell_w, cell_h));
        let bg = background_for(cell.attrs, theme);
        if bg != color32(&theme.background) {
            painter.rect_filled(cell_rect, 0.0, bg);
        }
        let ch = if cell.attrs.nondisplay { ' ' } else { cell.ch };
        painter.text(
            min,
            Align2::LEFT_TOP,
            ch.to_string(),
            font.clone(),
            foreground_for(cell.attrs, theme),
        );
        if cell.attrs.highlight == Highlight::Underscore {
            painter.line_segment(
                [
                    Pos2::new(cell_rect.left(), cell_rect.bottom() - 2.0),
                    Pos2::new(cell_rect.right(), cell_rect.bottom() - 2.0),
                ],
                Stroke::new(1.0, foreground_for(cell.attrs, theme)),
            );
        }
    }
    let cursor = tab.screen.cursor();
    let cursor_rect = Rect::from_min_size(
        Pos2::new(
            rect.left() + cursor.col as f32 * cell_w,
            rect.top() + cursor.row as f32 * cell_h,
        ),
        Vec2::new(cell_w, cell_h),
    );
    painter.rect_stroke(
        cursor_rect,
        0.0,
        Stroke::new(1.5, color32(&theme.cursor)),
        StrokeKind::Inside,
    );
}

fn foreground_for(attrs: CellAttrs, theme: &ThemeConfig) -> Color32 {
    if attrs.highlight == Highlight::Reverse {
        return color32(&theme.background);
    }
    if attrs.intensified {
        return color32(&theme.white);
    }
    match attrs.foreground {
        Color::Default => color32(&theme.foreground),
        Color::Black => color32(&theme.black),
        Color::Blue => color32(&theme.blue),
        Color::Red => color32(&theme.red),
        Color::Pink => color32(&theme.pink),
        Color::Green => color32(&theme.green),
        Color::Turquoise => color32(&theme.turquoise),
        Color::Yellow => color32(&theme.yellow),
        Color::White => color32(&theme.white),
    }
}

fn background_for(attrs: CellAttrs, theme: &ThemeConfig) -> Color32 {
    if attrs.highlight == Highlight::Reverse {
        return foreground_for(
            CellAttrs {
                highlight: Highlight::Default,
                ..attrs
            },
            theme,
        );
    }
    match attrs.background {
        Color::Default => color32(&theme.background),
        Color::Black => color32(&theme.black),
        Color::Blue => color32(&theme.blue),
        Color::Red => color32(&theme.red),
        Color::Pink => color32(&theme.pink),
        Color::Green => color32(&theme.green),
        Color::Turquoise => color32(&theme.turquoise),
        Color::Yellow => color32(&theme.yellow),
        Color::White => color32(&theme.white),
    }
}

fn color32(spec: &ColorSpec) -> Color32 {
    spec.rgb()
        .map(|(r, g, b)| Color32::from_rgb(r, g, b))
        .unwrap_or(Color32::WHITE)
}

fn upsert_profile(profiles: &mut Vec<HostProfile>, profile: HostProfile) {
    if let Some(existing) = profiles.iter_mut().find(|item| item.name == profile.name) {
        *existing = profile;
    } else {
        profiles.push(profile);
    }
}

fn load_profiles() -> Option<Vec<HostProfile>> {
    let path = config_dir()?.join("profiles.toml");
    let data = fs::read_to_string(path).ok()?;
    let store: ProfileStore = toml::from_str(&data).ok()?;
    Some(store.profiles)
}

fn save_profiles(profiles: &[HostProfile]) -> std::io::Result<()> {
    if let Some(dir) = config_dir() {
        fs::create_dir_all(&dir)?;
        let store = ProfileStore {
            profiles: profiles.to_vec(),
        };
        let data = toml::to_string_pretty(&store)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        fs::write(dir.join("profiles.toml"), data)?;
    }
    Ok(())
}

fn config_dir() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    #[cfg(target_os = "macos")]
    {
        Some(
            home.join("Library")
                .join("Application Support")
                .join("r3270"),
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        Some(home.join(".config").join("r3270"))
    }
}
