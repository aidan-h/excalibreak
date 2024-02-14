use std::path::Path;

pub use serde;
use serde::de::DeserializeOwned;
use serde::Serialize;
pub use tokio;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::io::{self, AsyncWriteExt};
pub use toml;

pub enum OneShotStatus<T> {
    Closed,
    /// When oneshot isn't done yet
    Empty,
    Value(T),
    /// When option is empty
    None,
}

pub fn receive_oneshot_rx<T>(
    rx: &mut Option<tokio::sync::oneshot::Receiver<T>>,
) -> OneShotStatus<T> {
    use tokio::sync::oneshot;
    if let Some(rx_inner) = rx.as_mut() {
        match rx_inner.try_recv() {
            Ok(value) => {
                *rx = None;
                return OneShotStatus::Value(value);
            }
            Err(err) => match err {
                oneshot::error::TryRecvError::Empty => {
                    return OneShotStatus::Empty;
                }
                oneshot::error::TryRecvError::Closed => {
                    *rx = None;
                    return OneShotStatus::Closed;
                }
            },
        };
    }
    OneShotStatus::None
}

pub async fn load_file(path: &str) -> io::Result<Vec<u8>> {
    let mut file = File::open(path).await?;

    let mut bytes = vec![];
    file.read_to_end(&mut bytes).await?;
    Ok(bytes)
}

pub async fn load_from_toml<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, String> {
    match File::open(path).await {
        Ok(mut file) => {
            let mut contents = String::new();
            if let Err(err) = file.read_to_string(&mut contents).await {
                return Err(err.to_string());
            }
            match toml::from_str(contents.as_str()) {
                Err(err) => Err(err.to_string()),
                Ok(val) => Ok(val),
            }
        }
        Err(err) => Err(format!("{err}")),
    }
}

pub fn save_to_toml<T: Serialize>(
    data: &T,
    path: String,
) -> tokio::sync::oneshot::Receiver<Result<(), String>> {
    let string: String = toml::to_string(data).unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        tx.send(match File::create(path).await {
            Ok(mut file) => match file.write_all(string.as_bytes()).await {
                Ok(()) => Ok(()),
                Err(err) => Err(format!("{err}")),
            },
            Err(err) => Err(format!("{err}")),
        })
        .unwrap();
    });
    rx
}
