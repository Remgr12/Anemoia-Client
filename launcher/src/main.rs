use eframe::egui;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use sysinfo::System;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Anemoia Launcher")
            .with_inner_size([640.0, 480.0])
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "Anemoia Launcher",
        options,
        Box::new(|_cc| Ok(Box::new(LauncherApp::new()))),
    )
}

// ── App ───────────────────────────────────────────────────────────────────────

struct LauncherApp {
    processes: Vec<ProcessEntry>,
    selected_pid: Option<u32>,

    injector_path: String,
    agent_path: String,
    client_path: String,

    status: String,
    status_ok: bool,

    log_lines: Arc<Mutex<Vec<String>>>,
    last_log_refresh: std::time::Instant,
}

#[derive(Clone)]
struct ProcessEntry {
    pid: u32,
    label: String,
}

impl LauncherApp {
    fn new() -> Self {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        let mut app = LauncherApp {
            processes: vec![],
            selected_pid: None,
            injector_path: exe_dir
                .join("anemoia-inject")
                .display()
                .to_string(),
            agent_path: exe_dir
                .join("libagent_loader.so")
                .display()
                .to_string(),
            client_path: exe_dir
                .join("libanemoia_client.so")
                .display()
                .to_string(),
            status: "Ready.".into(),
            status_ok: true,
            log_lines: Arc::new(Mutex::new(vec![])),
            last_log_refresh: std::time::Instant::now(),
        };
        app.refresh_processes();
        app
    }

    fn refresh_processes(&mut self) {
        let mut sys = System::new_all();
        sys.refresh_all();

        self.processes = sys
            .processes()
            .iter()
            .filter_map(|(pid, proc)| {
                let name = proc.name().to_string_lossy().to_lowercase();
                let cmd = proc
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ");
                let cmd_lower = cmd.to_lowercase();

                // Only actual MC JVM processes: java binary with the minecraft client jar
                if name != "java" || !cmd_lower.contains("com/mojang/minecraft") {
                    return None;
                }

                // Extract version from e.g. "minecraft-26.1.2-client.jar"
                let version = cmd
                    .split_whitespace()
                    .flat_map(|tok| tok.split(':'))
                    .find(|s| s.contains("minecraft-") && s.ends_with("-client.jar"))
                    .and_then(|s| s.split("minecraft-").nth(1))
                    .and_then(|s| s.strip_suffix("-client.jar"))
                    .unwrap_or("?");

                let label = format!("{:6}  Minecraft {}  (java)", pid.as_u32(), version);
                Some(ProcessEntry {
                    pid: pid.as_u32(),
                    label,
                })
            })
            .collect();

        // Sort by PID so the list is stable.
        self.processes.sort_by_key(|p| p.pid);

        if self.selected_pid.is_none() {
            if let Some(mc) = self.processes.first() {
                self.selected_pid = Some(mc.pid);
            }
        }
    }

    fn inject(&mut self) {
        let pid = match self.selected_pid {
            Some(p) => p,
            None => {
                self.set_status("No process selected.", false);
                return;
            }
        };

        let injector = self.injector_path.trim().to_owned();
        if injector.is_empty() || !std::path::Path::new(&injector).exists() {
            self.set_status(
                &format!("Injector not found: {}", injector),
                false,
            );
            return;
        }

        self.set_status(&format!("Injecting into PID {}...", pid), true);

        let log_ref = self.log_lines.clone();

        // Copy paths so the thread owns them.
        let injector_path = injector.clone();

        thread::spawn(move || {
            let result = Command::new(&injector_path)
                .arg("--pid")
                .arg(pid.to_string())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();

            match result {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let mut lines = log_ref.lock().unwrap();
                    for line in stdout.lines().chain(stderr.lines()) {
                        lines.push(line.to_owned());
                    }
                    if out.status.success() {
                        lines.push("[injector] OK".into());
                    } else {
                        lines.push(format!(
                            "[injector] exited with status {}",
                            out.status
                        ));
                    }
                }
                Err(e) => {
                    log_ref
                        .lock()
                        .unwrap()
                        .push(format!("[injector] spawn error: {}", e));
                }
            }
        });
    }

    fn refresh_client_log(&mut self) {
        if self.last_log_refresh.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.last_log_refresh = std::time::Instant::now();

        if let Ok(content) = std::fs::read_to_string("/tmp/anemoia_client.log") {
            let mut lines = self.log_lines.lock().unwrap();
            let new_lines: Vec<String> = content.lines().map(|l| l.to_owned()).collect();
            // Replace log pane with latest file contents.
            *lines = new_lines;
        }
    }

    fn set_status(&mut self, msg: &str, ok: bool) {
        self.status = msg.to_owned();
        self.status_ok = ok;
    }
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.refresh_client_log();
        ctx.request_repaint_after(Duration::from_millis(500));

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Anemoia Launcher");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Refresh").clicked() {
                        self.refresh_processes();
                    }
                });
            });
        });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let color = if self.status_ok {
                    egui::Color32::LIGHT_GREEN
                } else {
                    egui::Color32::LIGHT_RED
                };
                ui.colored_label(color, &self.status);
            });
        });

        egui::SidePanel::left("process_panel")
            .min_width(260.0)
            .show(ctx, |ui| {
                ui.heading("Minecraft Processes");
                ui.separator();

                if self.processes.is_empty() {
                    ui.label("No Minecraft processes found.");
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for proc in self.processes.clone() {
                                let selected = self.selected_pid == Some(proc.pid);
                                let label = egui::RichText::new(&proc.label).monospace();
                                if ui.selectable_label(selected, label).clicked() {
                                    self.selected_pid = Some(proc.pid);
                                }
                            }
                        });
                }

                ui.separator();

                let can_inject = self.selected_pid.is_some();
                if ui
                    .add_enabled(can_inject, egui::Button::new("Inject"))
                    .clicked()
                {
                    self.inject();
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.collapsing("Paths", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Injector:");
                    ui.text_edit_singleline(&mut self.injector_path);
                });
                ui.horizontal(|ui| {
                    ui.label("Agent .so:");
                    ui.text_edit_singleline(&mut self.agent_path);
                });
                ui.horizontal(|ui| {
                    ui.label("Client .so:");
                    ui.text_edit_singleline(&mut self.client_path);
                });
            });

            ui.separator();
            ui.label("Client log  (/tmp/anemoia_client.log)");

            let lines = self.log_lines.lock().unwrap().clone();
            let start = lines.len().saturating_sub(200);
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &lines[start..] {
                        ui.label(egui::RichText::new(line).monospace().small());
                    }
                });
        });
    }
}
