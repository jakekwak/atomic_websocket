use std::sync::Arc;

use helpers::{internal_client::AtomicClient, internal_server::AtomicServer};
use native_db::{native_db, Database, ToKey};
use native_model::{native_model, Model};
use serde::{Deserialize, Serialize};

pub mod native_db {
    pub use native_db::*;
}
pub mod native_model {
    pub use native_model::*;
}
pub mod bebop {
    pub use bebop::*;
}

pub use generated::schema::*;
pub use helpers::client_sender::{ClientSenders, ClientSendersTrait};
pub use helpers::common::{get_setting_by_key, set_setting};
pub use helpers::server_sender::{SenderStatus, ServerSender, ServerSenderTrait};
use tokio::sync::RwLock;

mod generated;
mod helpers;

#[derive(Serialize, Deserialize, Debug)]
#[native_model(id = 1004, version = 1)]
#[native_db]
pub struct Settings {
    #[primary_key]
    pub key: String,
    pub value: Vec<u8>,
}

pub struct AtomicWebsocket {}

impl AtomicWebsocket {
    pub async fn get_internal_client(db: Arc<RwLock<Database<'static>>>) -> AtomicClient {
        let mut server_sender = Arc::new(RwLock::new(ServerSender::new(db.clone(), "".into())));
        server_sender.regist(server_sender.clone()).await;

        let atomic_websocket: AtomicClient = AtomicClient { server_sender };
        atomic_websocket.initialize(db.clone()).await;
        atomic_websocket
    }

    pub async fn get_internal_server(addr: String) -> AtomicServer {
        AtomicServer::new(&addr).await
    }
}
