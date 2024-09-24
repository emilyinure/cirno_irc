use futures::StreamExt;
use irc::client::prelude::*;
use std::{
    sync::mpsc::{channel, Receiver, Sender},
    time::Duration,
};

struct Channel {
    messages: Vec<String>,
}

struct Server {
    motd: String,
    ip: String,
    receiver: Receiver<Message>,
    channels: Vec<Channel>,
}

impl Server {
    fn new(ip: &String) -> Self {
        let (sender, receiver) = channel();

        tokio::spawn(Server::update_loop(ip.clone(), sender));

        Self {
            ip: ip.clone(),
            receiver,
            motd: "".to_string(),
            channels: Vec::new(),
        }
    }

    async fn update_loop(server_ip: String, sender: Sender<Message>) -> Result<(), failure::Error> {
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
