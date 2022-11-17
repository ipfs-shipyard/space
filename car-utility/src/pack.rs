use anyhow::Result;
use cid::Cid;
use futures::StreamExt;
use iroh_car::{CarHeader, CarWriter};
use iroh_resolver::unixfs_builder::{File, FileBuilder};
use std::fs::File as FsFile;

use std::io::Write;
use std::path::PathBuf;

pub async fn pack(path: &PathBuf, output: &PathBuf) -> Result<()> {
    let file: File = FileBuilder::new().path(path).build().await?;

    let _root: Option<Cid> = None;
    let parts = { Box::pin(file.encode().await?) };
    tokio::pin!(parts);

    let mut cids = vec![];
    let mut datas = vec![];

    while let Some(part) = parts.next().await {
        let (cid, bytes, _links) = part?.into_parts();
        cids.push(cid);
        datas.push(bytes);
    }

    let mut buffer = vec![];
    let car_header = CarHeader::new_v1(cids.clone());
    let mut writer = CarWriter::new(car_header, &mut buffer);

    for (cid, data) in cids.into_iter().zip(datas.into_iter()) {
        writer.write(cid, data).await?;
    }

    writer.finish().await?;

    let mut f = FsFile::create(output).expect("failed to create file");
    f.write_all(&buffer)?;

    Ok(())
}
