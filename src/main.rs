#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use egui::{Layout, ViewportBuilder};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::symlink as create_symlink;
#[cfg(windows)]
use std::os::windows::fs::{symlink_dir, symlink_file};

// Struct definitions
struct EZSymlink {
    source_path: String,
    destination_path: String,
    status_message: String,
    status_color: egui::Color32,
    show_merge_warning: bool,
    show_error_dialog: bool,
    error_message: String,
    symlink_type: SymlinkType,
    recent_symlinks: Vec<(String, String)>,
}

#[derive(PartialEq, Clone, Copy)]
enum SymlinkType {
    Auto,
    File,
    Directory,
}

// Implementations
impl Default for EZSymlink {
    fn default() -> Self {
        Self {
            source_path: String::new(),
            destination_path: String::new(),
            status_message: String::new(),
            status_color: egui::Color32::GRAY,
            show_merge_warning: false,
            show_error_dialog: false,
            error_message: String::new(),
            symlink_type: SymlinkType::Auto,
            recent_symlinks: Vec::new(),
        }
    }
}

impl eframe::App for EZSymlink {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("EZSymlink");

            folder_input(
                ui,
                "Source:",
                &mut self.source_path,
                &mut self.status_message,
                &mut self.status_color,
            );
            folder_input(
                ui,
                "Destination:",
                &mut self.destination_path,
                &mut self.status_message,
                &mut self.status_color,
            );

            ui.horizontal(|ui| {
                ui.label("Symlink Type:");
                ui.radio_value(&mut self.symlink_type, SymlinkType::Auto, "Auto");
                ui.radio_value(&mut self.symlink_type, SymlinkType::File, "File");
                ui.radio_value(&mut self.symlink_type, SymlinkType::Directory, "Directory");
            });

            if ui.button("Create Symlink").clicked() {
                self.handle_symlink_creation();
            }

            let source_path = self.source_path.clone();
            if ui.button("Open Source Location").clicked() {
                self.open_file_explorer(&source_path);
            }

            let destination_path = self.destination_path.clone();
            if ui.button("Open Destination Location").clicked() {
                self.open_file_explorer(&destination_path);
            }

            if !self.status_message.is_empty() {
                ui.colored_label(self.status_color, &self.status_message);
            }

            ui.collapsing("Recent Symlinks", |ui| {
                for (src, dst) in &self.recent_symlinks {
                    ui.label(format!("{} -> {}", src, dst));
                }
            });
        });

        self.show_merge_warning_dialog(ctx);
        self.show_error_dialog(ctx);
    }
}

impl EZSymlink {
    fn copy_dir_contents(&self, src: &PathBuf, dst: &PathBuf) -> std::io::Result<()> {
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if ty.is_dir() {
                fs::create_dir_all(&dst_path)?;
                self.copy_dir_contents(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)?;
            }
        }
        Ok(())
    }

    fn create_symlink(&mut self) -> std::io::Result<()> {
        let source = PathBuf::from(&self.source_path);
        let destination = PathBuf::from(&self.destination_path);

        if let Some(parent) = destination.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        match self.symlink_type {
            SymlinkType::Auto => {
                if source.is_dir() {
                    #[cfg(windows)]
                    symlink_dir(&source, &destination)?;
                    #[cfg(unix)]
                    create_symlink(&source, &destination)?;
                } else {
                    #[cfg(windows)]
                    symlink_file(&source, &destination)?;
                    #[cfg(unix)]
                    create_symlink(&source, &destination)?;
                }
            }
            SymlinkType::File => {
                #[cfg(windows)]
                symlink_file(&source, &destination)?;
                #[cfg(unix)]
                create_symlink(&source, &destination)?;
            }
            SymlinkType::Directory => {
                #[cfg(windows)]
                symlink_dir(&source, &destination)?;
                #[cfg(unix)]
                create_symlink(&source, &destination)?;
            }
        }

        self.recent_symlinks
            .push((self.source_path.clone(), self.destination_path.clone()));
        if self.recent_symlinks.len() > 5 {
            self.recent_symlinks.remove(0);
        }

        Ok(())
    }

    fn handle_symlink_creation(&mut self) {
        if self.source_path.is_empty() || self.destination_path.is_empty() {
            self.set_error("Please select both source and destination.");
            return;
        }

        let source = PathBuf::from(&self.source_path);
        let destination = PathBuf::from(&self.destination_path);

        if !source.exists() {
            self.set_error("Source does not exist.");
            return;
        }

        if destination.exists() {
            self.show_merge_warning = true;
        } else {
            match self.create_symlink() {
                Ok(_) => self.set_success("Symlink created successfully!"),
                Err(e) => self.set_error(&format!("Error creating symlink: {}", e)),
            }
        }
    }

    fn merge_folders(&mut self) {
        let source = PathBuf::from(&self.source_path);
        let destination = PathBuf::from(&self.destination_path);

        if let Err(e) = self.copy_dir_contents(&destination, &source) {
            self.set_error(&format!("Error merging folders: {}", e));
        } else if let Err(e) = fs::remove_dir_all(&destination) {
            self.set_error(&format!(
                "Error removing original destination folder: {}",
                e
            ));
        } else {
            match self.create_symlink() {
                Ok(_) => self.set_success("Folders merged and symlink created successfully!"),
                Err(e) => self.set_error(&format!("Error creating symlink after merge: {}", e)),
            }
        }
    }

    fn set_error(&mut self, message: &str) {
        self.error_message = message.to_string();
        self.show_error_dialog = true;
    }

    fn set_success(&mut self, message: &str) {
        self.status_message = message.to_string();
        self.status_color = egui::Color32::GREEN;
    }

    fn show_merge_warning_dialog(&mut self, ctx: &egui::Context) {
        if self.show_merge_warning {
            let mut close_dialog = false;
            let mut action: Option<Box<dyn FnOnce(&mut Self)>> = None;

            egui::Window::new("Warning")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Destination already exists. Do you want to merge its contents into the source?");
                    ui.horizontal(|ui| {
                        if ui.button("Yes").clicked() {
                            action = Some(Box::new(Self::merge_folders));
                            close_dialog = true;
                        }
                        if ui.button("No").clicked() {
                            action = Some(Box::new(|s| s.set_error("Operation cancelled.")));
                            close_dialog = true;
                        }
                    });
                });

            if close_dialog {
                self.show_merge_warning = false;
                if let Some(action) = action {
                    action(self);
                }
            }
        }
    }

    fn show_error_dialog(&mut self, ctx: &egui::Context) {
        if self.show_error_dialog {
            egui::Window::new("Error")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(&self.error_message);
                    if ui.button("OK").clicked() {
                        self.show_error_dialog = false;
                        self.error_message.clear();
                    }
                });
        }
    }

    fn open_file_explorer(&mut self, path: &str) {
        let path = PathBuf::from(path);
        if path.exists() {
            #[cfg(target_os = "windows")]
            {
                if let Err(e) = Command::new("explorer").arg(&path).spawn() {
                    self.set_error(&format!("Failed to open file explorer: {}", e));
                }
            }
            #[cfg(target_os = "macos")]
            {
                if let Err(e) = Command::new("open").arg(&path).spawn() {
                    self.set_error(&format!("Failed to open Finder: {}", e));
                }
            }
            #[cfg(target_os = "linux")]
            {
                if let Err(e) = Command::new("xdg-open").arg(&path).spawn() {
                    self.set_error(&format!("Failed to open file manager: {}", e));
                }
            }
        } else {
            self.set_error("Path does not exist");
        }
    }
}

// Helper functions
fn clear_status(status_message: &mut String, status_color: &mut egui::Color32) {
    status_message.clear();
    *status_color = egui::Color32::GRAY;
}

fn folder_input(
    ui: &mut egui::Ui,
    label: &str,
    path: &mut String,
    status_message: &mut String,
    status_color: &mut egui::Color32,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        let available_width = ui.available_width();
        let text_edit_width = available_width - 70.0;

        ui.with_layout(Layout::right_to_left(eframe::emath::Align::Center), |ui| {
            if ui.button("Browse").clicked() {
                if let Some(selected_path) = rfd::FileDialog::new().pick_folder() {
                    *path = selected_path.display().to_string();
                    clear_status(status_message, status_color);
                }
            }

            let response = ui.add(
                egui::TextEdit::singleline(path)
                    .desired_width(text_edit_width)
                    .hint_text("Select a folder or file"),
            );

            if response.changed() {
                clear_status(status_message, status_color);
            }
        });
    });
}

// Main function
fn main() -> Result<(), eframe::Error> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        let file_path = &args[1];
        println!("Opening file: {}", file_path);
    }

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size([500.0, 300.0]),
        ..Default::default()
    };
    eframe::run_native(
        "EZSymlink",
        options,
        Box::new(|_cc| Ok(Box::new(EZSymlink::default()) as Box<dyn eframe::App>)),
    )
}
