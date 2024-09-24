use std::{
    sync::mpsc::{channel, Receiver, Sender},
    time::Duration,
};
use tokio::{runtime::Handle, time::sleep};

use eframe::{
    egui::{self, TextBuffer},
    CreationContext,
};
use futures::prelude::*;
use irc::client::prelude::*;

#[tokio::main]
async fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Confirm exit",
        options,
        Box::new(|_cc| Ok(Box::<Cirno>::new(Cirno::new()))),
    )
}

mod model;

struct Server {
    motd: String,
    server_ip: String,
    receiver: Receiver<Message>,
    messages: Vec<String>,
}

struct ConnectDialogue {}

struct Cirno {
    servers: Vec<Server>,
    server_ip: String,
    connected: bool,
    displaying_motd: bool,
    updating: bool,
}

impl Cirno {
    fn new() -> Self {
        Self {
            server_ip: String::from(""),
            connected: false,
            servers: Vec::new(),
            displaying_motd: false,
            updating: false,
        }
    }

    async fn connect(server_ip: String, sender: Sender<Message>) -> Result<(), failure::Error> {
        let config = Config {
            nickname: Some("cirno".to_owned()),
            server: Some(server_ip.to_owned()),
            channels: vec!["#test".to_owned()],
            ..Config::default()
        };

        let mut irc = Client::from_config(config).await?;
        irc.identify()?;

        let mut stream = irc.stream()?;

        while let Some(message) = stream.next().await.transpose()? {
            print!("{}", message);

            sender.send(message);
        }

        Ok(())
    }
}

fn update(ctx: egui::Context) {
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_millis(1)).await;
            ctx.request_repaint();
        }
    });
}

impl eframe::App for Cirno {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.updating {
            update(ctx.clone());
            self.updating = true;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if self.connected {
                    for server in &mut self.servers {
                        let message = server.receiver.recv_timeout(Duration::from_millis(1));

                        if let Ok(response) = message {
                            match &response.command {
                                &Command::PRIVMSG(ref chan, ref msg) => {
                                    let mut chat = String::new();
                                    if let Some(source_nickname) = response.source_nickname() {
                                        chat += source_nickname;
                                        chat += ": ";
                                    }

                                    chat += msg;
                                    server.messages.push(chat);
                                }
                                _ => (),
                            }
                        }
                        for message in &server.messages {
                            ui.label(message);
                        }
                    }
                }
            })
        });

        if !self.connected {
            egui::Window::new("Connect to server")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        ui.text_edit_singleline(&mut self.server_ip);
                        if ui.button("Connect").clicked() {
                            let server_ip = self.server_ip.clone();

                            self.servers.push(Server {
                                server_ip: self.server_ip.clone(),
                                receiver,
                                motd: String::from(""),
                                messages: Vec::new(),
                            });

                            tokio::spawn(Cirno::connect(server_ip.clone(), sender));
                            self.connected = true;
                        }
                    });
                });
        } else {
        }
    }
}
