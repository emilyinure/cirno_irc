use core::f32;
use eframe::egui::{self, Label, TextEdit};
use eframe::egui::{TextStyle, Ui};
use egui_extras::StripBuilder;
use futures::StreamExt;
use irc::client::prelude::*;
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::mpsc::{channel, Receiver},
    time::Duration,
};

enum MessageContent {
    Message(String, String),
    Join(String),
    Notice(String),
}

struct Channel {
    name: String,
    messages: Vec<MessageContent>,
    message: String,
    users: Vec<String>,
}

impl Channel {
    fn user_panel(&mut self, ui: &mut Ui) {
        egui::SidePanel::new(egui::panel::Side::Right, egui::Id::new("Users"))
            .frame(egui::Frame::default())
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        self.users.dedup();
                        for user in &self.users {
                            ui.label(user);
                        }
                    });
                });
            });
    }

    fn notice_display(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            let mut remove: Option<usize> = None;
            for (i, message) in self.messages.iter().enumerate() {
                match message {
                    MessageContent::Notice(notice) => {
                        ui.horizontal(|ui| {
                            ui.add(Label::new(notice).wrap());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center)
                                    .with_main_align(egui::Align::Min),
                                |ui| {
                                    if ui.button("x").clicked() {
                                        remove = Some(i);
                                    }
                                },
                            );
                        });
                        ui.separator();
                    }
                    _ => {}
                };
            }
            if let Some(remove) = remove {
                self.messages.remove(remove);
            }
        });
    }

    fn messaging_display(&mut self, ui: &mut Ui, sender: &irc::client::Sender, nickname: &String) {
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add(
                TextEdit::singleline(&mut self.message)
                    .desired_width(ui.available_width() - ui.style().spacing.item_spacing.x * 2.0),
            );
            ui.input(|input| {
                if input.key_pressed(egui::Key::Enter) {
                    match sender.send_privmsg(&self.name, &self.message) {
                        Ok(_) => self.messages.push(MessageContent::Message(
                            nickname.clone(),
                            self.message.clone(),
                        )),
                        Err(_) => {}
                    };
                    self.message = String::from("");
                }
            });
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        for message in &self.messages {
                            match message {
                                MessageContent::Message(source, body) => {
                                    ui.label(format!("{}: {}", &source, &body));
                                }
                                MessageContent::Join(source) => {
                                    ui.label(format!("{} has joined", source));
                                }
                                _ => {}
                            };
                        }
                    });
                });
        });
    }

    pub fn update(&mut self, ui: &mut Ui, sender: &irc::client::Sender, nickname: &String) {
        self.user_panel(ui);
        self.notice_display(ui);
        self.messaging_display(ui, sender, nickname);
    }
}

impl Channel {
    fn new(name: &String) -> Self {
        Self {
            users: Vec::new(),
            name: name.clone(),
            messages: Vec::new(),
            message: String::from(""),
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
    nickname: String,
    motd: Vec<String>,
    sender: Sender,
    pub ip: String,
    receiver: Receiver<Message>,
    channels: HashMap<u64, Channel>,
    connected: bool,
    selected_channel: u64,
    join_dialogue: ChannelJoinDialogue,
    displaying_motd: bool,
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
            displaying_motd: false,
            nickname: String::from("cirno"),
            join_dialogue: ChannelJoinDialogue {
                open: false,
                buffer: String::from(""),
            },
            selected_channel: 0,
            ip: ip.clone(),
            receiver: message_receiver,
            sender,
            motd: Vec::new(),
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
                egui::ScrollArea::horizontal().show(ui, |ui| {
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

    fn motd_display(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        if self.displaying_motd {
            let size = ui.available_size().y;
            egui::Window::new("MOTD")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    egui::ScrollArea::new(true)
                        .auto_shrink([false, true])
                        .max_height(size * 0.5)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                for i in &self.motd {
                                    ui.label(i);
                                }
                            });
                        });
                    ui.vertical_centered(|ui| {
                        if ui.button("Close").clicked() {
                            self.displaying_motd = false;
                        }
                    });
                });
        }
    }

    fn event_loop(&mut self) {
        while let Ok(response) = self.receiver.recv_timeout(Duration::from_millis(1)) {
            match &response.command {
                &Command::PRIVMSG(ref chan, ref msg) => {
                    let mut nickname = String::new();
                    if let Some(source_nickname) = response.source_nickname() {
                        nickname = source_nickname.to_string();
                    }

                    if let Some(channel) = self.get_channel(chan) {
                        channel
                            .messages
                            .push(MessageContent::Message(nickname, msg.clone()));
                    } else {
                        if let Some(channel) = self.create_channel(chan) {
                            channel
                                .messages
                                .push(MessageContent::Message(nickname, msg.clone()));
                        }
                    }
                }
                &Command::JOIN(ref chan, _, ref users) => match self.get_channel(chan) {
                    Some(channel) => {
                        if let Some(source_nickname) = response.source_nickname() {
                            channel
                                .messages
                                .push(MessageContent::Join(source_nickname.to_string()));
                            //channel.users.push(source_nickname.to_string());
                        }
                    }
                    None => {
                        if let Some(channel) = self.create_channel(chan) {
                            if let Some(source_nickname) = response.source_nickname() {
                                channel
                                    .messages
                                    .push(MessageContent::Join(source_nickname.to_string()));
                                //channel.users.push(source_nickname.to_string());
                            }
                        }
                    }
                },
                &Command::Response(ref response, ref users) => match response {
                    Response::RPL_NAMREPLY => {
                        let mut channel: Option<&mut Channel> = None;
                        for i in users {
                            if let Some(ref mut channel) = channel {
                                for name in i.split_whitespace() {
                                    channel.users.push(name.to_string());
                                }
                            }
                            if i.starts_with('#') {
                                channel = self.get_channel(i);
                            }
                        }
                    }
                    Response::RPL_MOTD => {
                        self.motd.push(match users.last() {
                            Some(val) => val.clone(),
                            None => String::new(),
                        });
                        self.displaying_motd = true;
                    }
                    _ => {}
                },
                &Command::NOTICE(ref target, ref msg) => {
                    let mut nickname = String::new();
                    if let Some(source_nickname) = response.source_nickname() {
                        nickname = source_nickname.to_string();
                    }

                    if let Some(channel) = self.get_channel(target) {
                        channel.messages.push(MessageContent::Notice(msg.clone()));
                    } else {
                        if let Some(channel) = self.create_channel(target) {
                            channel.messages.push(MessageContent::Notice(msg.clone()));
                        }
                    }
                }
                &Command::NAMES(ref channels, ref users) => {
                    println!("RECIEVED NAMES {users:?}");
                }
                &Command::USERHOST(ref users) => {
                    println!("RECIEVED NAMES {users:?}");
                }
                _ => (),
            }
        }
    }

    pub fn update(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        if self.connected {
            self.event_loop();
            self.motd_display(ctx, ui);
            self.channel_tabs(ctx, ui);

            ui.separator();
            if let Some(current_channel) = self.channels.get_mut(&self.selected_channel) {
                current_channel.update(ui, &self.sender, &self.nickname);
            }
        }
    }
}
