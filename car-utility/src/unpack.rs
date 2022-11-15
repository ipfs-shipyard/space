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

    let mut files: Vec<_> = car_reader.stream().try_collect().await.unwrap();
    // I shouldn't be writing the last block to the file..I think this might be the header?
    // Need to figure out a better way to do this
    files.pop();

    for (_cid, data) in files {
        output_file.write_all(&data).await?;
    }
    Ok(())
}
