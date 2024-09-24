use irc::client::prelude::*;
use std::{
    sync::mpsc::{channel, Receiver, Sender},
    time::Duration,
};

struct Server {
    motd: String,
    server_ip: String,
    receiver: Receiver<Message>,
    messages: Vec<String>,
}


