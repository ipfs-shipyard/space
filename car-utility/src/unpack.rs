use anyhow::Result;
use futures::TryStreamExt;
use iroh_car::CarReader;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;

pub async fn unpack(path: &PathBuf, output: &PathBuf) -> Result<()> {
    let file = File::open(path).await?;
    let buf_reader = BufReader::new(file);

    let car_reader = CarReader::new(buf_reader).await?;
    let mut output_file = File::create(output).await?;

    let files: Vec<_> = car_reader.stream().try_collect().await.unwrap();
    for (_cid, data) in files {
        output_file.write_all(&data).await?;
    }
    output_file.flush().await?;
    Ok(())
}
