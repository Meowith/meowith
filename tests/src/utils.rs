use http::Extensions;
use log::info;
use reqwest::{Body, Request, Response};
use reqwest_middleware::Result;
use reqwest_middleware::{Middleware, Next};
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use tokio::io::AsyncReadExt;
use tokio_util::codec::{BytesCodec, FramedRead};

pub fn compare_files<P: AsRef<Path>>(
    file_path1: P,
    file_path2: P,
    range: Option<(u64, u64)>,
) -> io::Result<bool> {
    let file1 = File::open(&file_path1)?;
    let file2 = File::open(&file_path2)?;

    let metadata1 = file1.metadata()?;
    let metadata2 = file2.metadata()?;

    let (start, end) = match range {
        Some((start, end)) => {
            if start >= end || end > metadata1.len() {
                return Ok(false);
            }
            (start, end)
        }
        None => (0, metadata1.len()),
    };

    if metadata1.len() < (end - start) || metadata2.len() < (end - start) {
        return Ok(false);
    }

    let mut reader1 = BufReader::new(file1);
    let mut reader2 = BufReader::new(file2);

    reader1.seek(SeekFrom::Start(start))?;

    let length = (end - start) as usize;
    let mut buffer1 = vec![0; std::cmp::min(length, 4096)];
    let mut buffer2 = vec![0; std::cmp::min(length, 4096)];

    let mut total_bytes_read = 0;

    loop {
        let bytes_to_read = std::cmp::min(buffer1.len(), length - total_bytes_read);
        if bytes_to_read == 0 {
            break;
        }

        let bytes_read1 = reader1.read(&mut buffer1[..bytes_to_read])?;
        let bytes_read2 = reader2.read(&mut buffer2[..bytes_read1])?;

        if bytes_read1 != bytes_read2 || buffer1[..bytes_read1] != buffer2[..bytes_read1] {
            return Ok(false);
        }

        if bytes_read1 == 0 {
            break;
        }

        total_bytes_read += bytes_read1;
    }

    Ok(true)
}

pub fn file_to_body(file: tokio::fs::File) -> Body {
    let stream = FramedRead::new(file, BytesCodec::new());
    Body::wrap_stream(stream)
}

pub struct Logger;

#[async_trait::async_trait]
impl Middleware for Logger {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> Result<Response> {
        info!("Request started {:?}", req);
        let res = next.run(req, extensions).await;
        info!("Result: {:?}", res);
        res
    }
}

pub async fn test_files(template: &str, target: &str, range: Option<(u64, u64)>) {
    let comparison = compare_files(template, target, range).expect("Unable to compare files");

    let mut file = tokio::fs::File::open(template).await.unwrap();
    let mut buffer = [0; 100];
    let n = file.read(&mut buffer).await.unwrap();
    println!(
        "len={} start={}",
        file.metadata().await.unwrap().len(),
        String::from_utf8_lossy(&buffer[..n])
    );

    let mut file = tokio::fs::File::open(target).await.unwrap();
    let mut buffer = [0; 100];
    let n = file.read(&mut buffer).await.unwrap();
    println!(
        "len={} start={}",
        file.metadata().await.unwrap().len(),
        String::from_utf8_lossy(&buffer[..n])
    );

    assert!(comparison);
}
