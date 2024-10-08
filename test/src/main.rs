use std::{
    env::current_dir,
    error::Error,
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::Duration,
};

use atomic_websocket::{
    client_sender::ServerOptions,
    external::native_db::{Builder, Database, Models},
    schema::ServerConnectInfo,
    server_sender::{ClientOptions, SenderStatus},
    AtomicWebsocket, Settings,
};
use tokio::{
    sync::{broadcast::Receiver, RwLock},
    time::sleep,
};

#[tokio::main]
async fn main() {
    let config_str = include_str!("log_config.yml");
    let config = serde_yaml::from_str(config_str).unwrap();
    log4rs::init_raw_config(config).unwrap();

    let port = "9000";
    let address: String = format!("0.0.0.0:{}", port);

    tokio::spawn(server_start(address.clone()));
    tokio::spawn(internal_client_start(port));

    // tokio::spawn(outer_client_start());

    loop {
        sleep(Duration::from_secs(100)).await;
    }
}

async fn server_start(address: String) {
    let option = ServerOptions::default();

    let atomic_server = AtomicWebsocket::get_internal_server(address, option).await;
    let handle_message_receiver = atomic_server.get_handle_message_receiver().await;

    tokio::spawn(receive_server_handle_message(handle_message_receiver));
}

pub async fn receive_server_handle_message(mut receiver: Receiver<(Vec<u8>, String)>) {
    while let Ok(message) = receiver.recv().await {
        log::debug!("Message: {:?}", message);
    }
}

async fn outer_client_start() {
    let current_path = match get_db_path() {
        Ok(path) => path,
        Err(error) => {
            log::error!("Failed to get db path {error:?}");
            return;
        }
    };
    let models = make_models();
    let db = make_db(models, current_path);
    let db = Arc::new(RwLock::new(db));

    let mut client_options = ClientOptions::default();
    client_options.url = "example.com/websocket".into();
    let atomic_client = AtomicWebsocket::get_outer_client(db.clone(), client_options).await;

    let status_receiver = atomic_client.get_status_receiver().await;
    let handle_message_receiver = atomic_client.get_handle_message_receiver().await;

    tokio::spawn(receive_status(status_receiver));
    tokio::spawn(receive_handle_message(handle_message_receiver));

    let _ = atomic_client.get_outer_connect(db.clone()).await;
}

async fn internal_client_start(port: &str) {
    let current_path = match get_db_path() {
        Ok(path) => path,
        Err(error) => {
            log::error!("Failed to get db path {error:?}");
            return;
        }
    };
    let models = make_models();
    let db = make_db(models, current_path);
    let db = Arc::new(RwLock::new(db));

    let client_options = ClientOptions::default();
    let atomic_client = AtomicWebsocket::get_internal_client(db.clone(), client_options).await;

    let status_receiver = atomic_client.get_status_receiver().await;
    let handle_message_receiver = atomic_client.get_handle_message_receiver().await;

    tokio::spawn(receive_status(status_receiver));
    tokio::spawn(receive_handle_message(handle_message_receiver));

    let _ = atomic_client
        .get_internal_connect(
            Some(ServerConnectInfo {
                current_ip: "192.168.200.194",
                broadcast_ip: "192.168.200.255",
                gateway_ip: "192.168.200.254",
                server_ip: "",
                port,
            }),
            db.clone(),
        )
        .await;
}

pub async fn receive_status(mut receiver: Receiver<SenderStatus>) {
    while let Ok(status) = receiver.recv().await {
        log::debug!("Status: {:?}", status);
        if status == SenderStatus::Disconnected {
            log::debug!("Disconnected");
        }
        if status == SenderStatus::Connected {
            log::debug!("Connected");
        }
    }
}

pub async fn receive_handle_message(mut receiver: Receiver<Vec<u8>>) {
    while let Ok(message) = receiver.recv().await {
        log::debug!("Message: {:?}", message);
    }
}

pub fn get_db_path() -> Result<PathBuf, Box<dyn Error>> {
    let mut current_path = PathBuf::from(current_dir().unwrap());
    current_path.push("database.redb");
    Ok(current_path)
}

pub fn make_models() -> &'static Models {
    static BUILDER: OnceLock<Models> = OnceLock::new();
    BUILDER.get_or_init(|| {
        let mut models = Models::new();
        if let Err(_) = models.define::<Settings>() {
            log::error!("Failed to define ClientTable");
        };
        models
    })
}

pub fn make_db(models: &'static Models, path: PathBuf) -> Database<'static> {
    let mut db = None;
    while db.is_none() {
        match Builder::new().create(models, &path) {
            Ok(database) => {
                db = Some(database);
            }
            Err(error) => {
                panic!("Failed to create db {error:?}");
            }
        }
    }
    db.unwrap()
}
