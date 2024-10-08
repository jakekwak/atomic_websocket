use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use bebop::Record;
use tokio::{
    sync::{broadcast, mpsc::Sender, RwLock},
    time::sleep,
};
use tokio_tungstenite::tungstenite::Message;

use crate::{
    helpers::{
        common::make_disconnect_message,
        traits::{date_time::now, StringUtil},
    },
    log_debug, log_error,
    schema::Data,
};

use super::common::make_expired_output_message;

pub struct ClientSenders {
    lists: Vec<ClientSender>,
    handle_message_sx: broadcast::Sender<(Vec<u8>, String)>,
}

impl ClientSenders {
    pub fn new() -> Self {
        let (handle_message_sx, _) = broadcast::channel(32);
        Self {
            lists: Vec::new(),
            handle_message_sx,
        }
    }

    pub async fn add(&mut self, peer: String, sx: Sender<Message>) {
        let list = self.lists.iter().position(|x| x.peer == peer);
        log_debug!("Add peer: {:?}, list: {:?}", peer, list);
        match list {
            Some(index) => {
                let list = self.lists.get_mut(index).unwrap();
                let _ = list.sx.send(make_disconnect_message(&peer)).await;
                list.sx = sx;
            }
            None => self.lists.push(ClientSender {
                peer,
                sx,
                send_time: 0,
            }),
        };
    }

    pub fn get_handle_message_receiver(&self) -> broadcast::Receiver<(Vec<u8>, String)> {
        self.handle_message_sx.subscribe()
    }

    pub async fn send_handle_message(&self, data: Vec<u8>, peer: String) {
        let handle_message_sx = self.handle_message_sx.clone();
        let _ = handle_message_sx.send((data, peer));
    }

    pub fn check_client_send_time(&mut self) {
        let now = now().timestamp();
        let mut remove_list = Vec::new();
        for (index, client) in self.lists.iter().enumerate() {
            if client.send_time + 30 < now {
                remove_list.push(index);
            }
        }
        for index in remove_list {
            self.lists.remove(index);
        }
    }

    pub fn remove(&mut self, peer: String) {
        let list = self.lists.iter().position(|x| x.peer == peer);
        log_debug!("Remove peer: {:?}, list: {:?}", peer, list);
        match list {
            Some(index) => {
                self.lists.remove(index);
            }
            None => {}
        };
    }

    pub async fn send(&mut self, peer: String, message: Message) -> bool {
        let mut result = true;
        for client in self.lists.iter_mut() {
            if client.peer == peer {
                let sender = client.sx.clone();
                let mut is_send = false;
                let mut count = 0;
                while is_send == false {
                    match sender.send(message.clone()).await {
                        Ok(_) => {
                            is_send = true;
                            client.write_time();
                        }
                        Err(e) => {
                            if count > 5 {
                                result = false;
                                break;
                            }
                            log_error!("Error client sending message: {:?}", e);
                            count += 1;
                        }
                    }
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
        result
    }
}

#[async_trait]
pub trait ClientSendersTrait {
    async fn add(&self, peer: String, sx: Sender<Message>);
    async fn get_handle_message_receiver(&self) -> broadcast::Receiver<(Vec<u8>, String)>;
    fn send_handle_message(&self, data: Data<'_>, peer: String);
    async fn send(&self, peer: String, message: Message);
    async fn expire_send(&self, peer_list: Vec<String>);
}

#[async_trait]
impl ClientSendersTrait for Arc<RwLock<ClientSenders>> {
    async fn add(&self, peer: String, sx: Sender<Message>) {
        let clone = self.clone();
        clone.write().await.add(peer, sx).await;
        drop(clone);
    }

    async fn get_handle_message_receiver(&self) -> broadcast::Receiver<(Vec<u8>, String)> {
        let clone = self.read().await;
        clone.get_handle_message_receiver()
    }

    fn send_handle_message(&self, data: Data<'_>, peer: String) {
        let clone = self.clone();

        let mut buf = Vec::new();
        data.serialize(&mut buf).unwrap();
        tokio::spawn(async move {
            let _ = clone.write().await.send_handle_message(buf, peer).await;
            drop(clone);
        });
    }

    async fn send(&self, peer: String, message: Message) {
        let clone = self.clone();
        let mut clone = clone.write().await;
        let result = clone.send(peer.copy_string(), message).await;

        if result == false {
            clone.remove(peer);
        }
        drop(clone);
    }

    async fn expire_send(&self, peer_list: Vec<String>) {
        let clone = self.clone();
        let lists = &clone.read().await.lists.clone();
        drop(clone);
        for peer in lists {
            if !peer_list.contains(&peer.peer) {
                self.send(peer.peer.copy_string(), make_expired_output_message())
                    .await;
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ClientSender {
    peer: String,
    sx: Sender<Message>,
    send_time: i64,
}

impl ClientSender {
    pub fn write_time(&mut self) {
        self.send_time = now().timestamp();
    }
}
