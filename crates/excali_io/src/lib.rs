pub use tokio;
use tokio::fs::File;
use tokio::io;
use tokio::io::AsyncReadExt;

pub async fn load_file(path: &str) -> io::Result<Vec<u8>> {
    let mut file = File::open(path).await?;

    let mut bytes = vec![];
    file.read_to_end(&mut bytes).await?;
    Ok(bytes)
}
