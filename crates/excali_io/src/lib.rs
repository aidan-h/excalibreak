use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

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

/// simplifies oneshot channel handling while updating an optional receiver
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

pub trait SerializeKey: Eq + std::hash::Hash + Sized {
    type Err;
    fn as_key(&self) -> String;
    fn from_key(key: &str) -> Result<Self, Self::Err>;

    fn serialize_hash_map<V: Clone>(hash_map: &HashMap<Self, V>) -> HashMap<String, V> {
        let mut new_map = HashMap::<String, V>::new();
        for (key, value) in hash_map.iter() {
            new_map.insert(key.as_key(), value.clone());
        }
        new_map
    }

    fn deserialize_hash_map<V: Clone>(
        hash_map: &HashMap<String, V>,
    ) -> Result<HashMap<Self, V>, Self::Err> {
        let mut new_map = HashMap::<Self, V>::new();
        for (key, value) in hash_map.iter() {
            new_map.insert(Self::from_key(key)?, value.clone());
        }
        Ok(new_map)
    }
}

#[derive(Debug)]
pub enum FromKeyError<T: FromStr>
where
    <T as FromStr>::Err: std::fmt::Debug,
{
    FromStr(<T as FromStr>::Err),
    SliceDoesntFit,
}
//TODO extend impl to SMatrix
#[cfg(feature = "nalgebra")]
impl<T: Eq + FromStr + ToString + core::fmt::Debug + std::hash::Hash + nalgebra::Scalar>
    SerializeKey for nalgebra::Vector2<T>
where
    <T as FromStr>::Err: std::fmt::Debug,
{
    type Err = FromKeyError<T>;
    fn as_key(&self) -> String {
        let mut out = String::new();
        let mut values = self.iter().peekable();
        while let Some(value) = values.next() {
            if values.peek().is_some() {
                out += &(value.to_string() + " ");
            } else {
                out += &value.to_string();
            }
        }
        out
    }

    fn from_key(key: &str) -> Result<Self, FromKeyError<T>> {
        let mut values = Vec::<T>::new();
        for string in key.split(' ') {
            match T::from_str(string) {
                Ok(value) => {
                    values.push(value);
                }
                Err(err) => return Err(FromKeyError::FromStr(err)),
            }
        }
        match <[T; 2]>::try_from(values) {
            Ok(value) => Ok(value.into()),
            Err(_) => Err(FromKeyError::SliceDoesntFit),
        }
    }
}
