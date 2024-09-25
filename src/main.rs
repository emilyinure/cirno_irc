mod model;
mod ui;

use model::server::Server;

use eframe::egui::{self, Id};

#[tokio::main]
async fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Cirno",
        options,
        Box::new(|_cc| Ok(Box::<Cirno>::new(Cirno::new()))),
    )
}

struct Cirno {
    servers: Vec<Server>,
    server_ip: String,
    selected_server: usize,
    connection_dialogue: bool,
}

impl Cirno {
    fn new() -> Self {
        Self {
            server_ip: String::from(""),
            servers: Vec::new(),
            selected_server: 0,
            connection_dialogue: false,
        }
    }
    fn connect_to_server(&mut self, ctx: &egui::Context) {
        egui::Window::new("Connect to server")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.text_edit_singleline(&mut self.server_ip);
                if ui.button("Connect").clicked() {
                    if let Some(server) = Server::new(&self.server_ip, ctx.clone()) {
                        self.servers.push(server);
                    }
                    self.connection_dialogue = false;
                }
            });
    }
}

impl eframe::App for Cirno {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.connection_dialogue {
            self.connect_to_server(ctx);
        }

        egui::SidePanel::new(egui::panel::Side::Left, Id::new("Servers")).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    if ui.button("Connect to new server").clicked() {
                        self.connection_dialogue = true;
                    }
                    for (i, server) in self.servers.iter().enumerate() {
                        if ui
                            .selectable_label(i == self.selected_server, server.ip.clone())
                            .clicked()
                        {
                            self.selected_server = i;
                        };
                    }
                });
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            for (i, server) in self.servers.iter_mut().enumerate() {
                if i == self.selected_server {
                    server.update(ctx, ui);
                }
            }
        });

        if self.servers.is_empty() {
        } else {
        }
    }
}
