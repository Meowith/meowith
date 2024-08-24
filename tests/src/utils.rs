use http::Extensions;
use log::info;
use reqwest::{Body, Request, Response};
use reqwest_middleware::Result;
use reqwest_middleware::{Middleware, Next};
use std::io::{self, BufReader, Read};
use std::path::Path;
use tokio_util::codec::{BytesCodec, FramedRead};

pub fn compare_files<P: AsRef<Path>>(file_path1: P, file_path2: P) -> io::Result<bool> {
    let file1 = std::fs::File::open(&file_path1)?;
    let file2 = std::fs::File::open(&file_path2)?;

    if file1.metadata()?.len() != file2.metadata()?.len() {
        return Ok(false);
    }

    let mut reader1 = BufReader::new(file1);
    let mut reader2 = BufReader::new(file2);

    let mut buffer1 = [0; 4096];
    let mut buffer2 = [0; 4096];

    loop {
        let bytes_read1 = reader1.read(&mut buffer1)?;
        let bytes_read2 = reader2.read(&mut buffer2)?;

        if bytes_read1 != bytes_read2 {
            return Ok(false);
        }

        if buffer1[..bytes_read1] != buffer2[..bytes_read1] {
            return Ok(false);
        }

        if bytes_read1 == 0 {
            break;
        }
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
