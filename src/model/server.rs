use eframe::egui::Ui;
use eframe::egui::{self, TextEdit};
use futures::StreamExt;
use irc::client::prelude::*;
use std::{
    borrow::BorrowMut,
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    ptr::hash,
    sync::mpsc::{channel, Receiver},
    time::Duration,
};

struct Channel {
    name: String,
    messages: Vec<String>,
}

impl Channel {
    pub fn update(&mut self, ui: &mut Ui, sender: &irc::client::Sender) {
        ui.vertical(|ui| {
            for message in &self.messages {
                ui.label(message);
            }
            let mut text = String::new();
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add(TextEdit::singleline(&mut text).desired_width(ui.available_width()));
            });
        });
    }
}

impl Channel {
    fn new(name: &String) -> Self {
        Self {
            name: name.clone(),
            messages: Vec::new(),
        }
    }
}

struct ChannelJoinDialogue {
    open: bool,
    buffer: String,
}

impl ChannelJoinDialogue {
    fn open(&mut self) {
        self.open = true;
    }
    fn update(&mut self, ctx: &egui::Context, ui: &mut Ui) -> bool {
        let mut res = false;
        if self.open {
            egui::Window::new("Connect to channel")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        ui.text_edit_singleline(&mut self.buffer);
                        if ui.button("Connect").clicked() {
                            self.open = false;
                            res = true;
                        }
                    });
                });
        }
        res
    }
}

pub struct Server {
    messages: Vec<String>,
    motd: String,
    sender: Sender,
    pub ip: String,
    receiver: Receiver<Message>,
    channels: HashMap<u64, Channel>,
    connected: bool,
    selected_channel: u64,
    join_dialogue: ChannelJoinDialogue,
}

impl Server {
    async fn start(
        server_ip: String,
        sender_sender: std::sync::mpsc::Sender<irc::client::Sender>,
        message_sender: std::sync::mpsc::Sender<Message>,
        ctx: egui::Context,
    ) -> Result<(), failure::Error> {
        let config = Config {
            nickname: Some("cirno".to_owned()),
            server: Some(server_ip.to_owned()),
            channels: vec!["#test".to_owned()],
            ..Config::default()
        };

        let mut irc = Client::from_config(config).await?;
        irc.identify()?;
        sender_sender.send(irc.sender());

        let mut stream = irc.stream()?;

        while let Some(message) = stream.next().await.transpose()? {
            print!("{}", message);

            ctx.request_repaint();
            message_sender.send(message);
        }

        Ok(())
    }

    pub fn new(ip: &String, ctx: egui::Context) -> Option<Self> {
        let (sender_sender, sender_receiver) = channel();
        let (message_sender, message_receiver) = channel();

        tokio::spawn(Server::start(
            ip.clone(),
            sender_sender,
            message_sender,
            ctx,
        ));
        let sender = match sender_receiver.recv() {
            Ok(val) => val,
            Err(_) => return None,
        };

        Some(Self {
            join_dialogue: ChannelJoinDialogue {
                open: false,
                buffer: String::from(""),
            },
            selected_channel: 0,
            messages: Vec::new(),
            ip: ip.clone(),
            receiver: message_receiver,
            sender,
            motd: "".to_string(),
            channels: HashMap::new(),
            connected: true,
        })
    }

    fn create_channel(&mut self, channel: &String) -> Option<&mut Channel> {
        let mut hasher = DefaultHasher::new();
        channel.hash(&mut hasher);
        let channel_hash = hasher.finish();
        self.channels.insert(channel_hash, Channel::new(channel));
        println!("creating channel");
        self.get_channel(channel)
    }

    fn get_channel(&mut self, channel: &String) -> Option<&mut Channel> {
        let mut hasher = DefaultHasher::new();
        channel.hash(&mut hasher);
        let channel_hash = hasher.finish();
        self.channels.get_mut(&channel_hash)
    }

    fn channel_tabs(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        if self.join_dialogue.update(ctx, ui) {
            self.sender.send_join(&self.join_dialogue.buffer);
        }
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Join").clicked() {
                    self.join_dialogue.buffer = String::from("");
                    self.join_dialogue.open = true;
                    self.join_dialogue.open();
                }
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.horizontal(|ui| {
                            for (hash, channel) in &self.channels {
                                if ui
                                    .selectable_label(self.selected_channel == *hash, &channel.name)
                                    .clicked()
                                {
                                    self.selected_channel = *hash;
                                }
                            }
                        });
                    });
                });
            });
        });
    }

    pub fn update(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        if self.connected {
            self.channel_tabs(ctx, ui);
            let message = self.receiver.recv_timeout(Duration::from_millis(1));

            if let Ok(response) = message {
                match &response.command {
                    &Command::PRIVMSG(ref chan, ref msg) => {
                        let mut chat = String::new();
                        if let Some(source_nickname) = response.source_nickname() {
                            chat += source_nickname;
                            chat += ": ";
                        }

                        chat += msg;
                        if let Some(channel) = self.get_channel(chan) {
                            channel.messages.push(chat);
                        } else {
                            if let Some(channel) = self.create_channel(chan) {
                                channel.messages.push(chat);
                            }
                        }
                    }
                    &Command::JOIN(ref chan, _, _) => {
                        self.create_channel(chan);
                    }
                    _ => (),
                }
            }
            ui.separator();
            if let Some(current_channel) = self.channels.get_mut(&self.selected_channel) {
                current_channel.update(ui, &self.sender);
            }
        }
    }
}
