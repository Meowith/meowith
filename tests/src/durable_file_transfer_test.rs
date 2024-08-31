use crate::file_transfer_test::download_file;
use crate::utils::{file_to_body_ranged, file_to_body_ranged_await, test_files, Logger};
use data::dto::entity::{
    AppDto, BucketDto, UploadSessionRequest, UploadSessionResumeRequest,
    UploadSessionResumeResponse, UploadSessionStartResponse,
};
use http::header::{AUTHORIZATION, CONTENT_LENGTH};
use log::info;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use std::ops::Range;
use std::time::Duration;
use tokio::fs::File;
use tokio::time::sleep;
use uuid::Uuid;

async fn start_upload_session(
    path: &str,
    remote_path: &str,
    node: &str,
    bucket_id: Uuid,
    app_id: Uuid,
    token: &str,
    client: &ClientWithMiddleware,
) -> UploadSessionStartResponse {
    let file = File::open(path).await.unwrap();
    let size = file.metadata().await.unwrap().len();
    let req = UploadSessionRequest { size };

    client
        .post(format!(
            "http://{node}/api/file/upload/durable/{app_id}/{bucket_id}/{remote_path}"
        ))
        .json(&req)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .expect("")
        .json::<UploadSessionStartResponse>()
        .await
        .expect("")
}

async fn resume_upload_session(
    session_id: &str,
    node: &str,
    bucket_id: Uuid,
    app_id: Uuid,
    token: &str,
    client: &ClientWithMiddleware,
) -> UploadSessionResumeResponse {
    let req = UploadSessionResumeRequest {
        session_id: session_id.to_string().parse().unwrap(),
    };

    client
        .post(format!(
            "http://{node}/api/file/upload/resume/{app_id}/{bucket_id}"
        ))
        .json(&req)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .expect("")
        .json::<UploadSessionResumeResponse>()
        .await
        .expect("")
}

#[allow(clippy::too_many_arguments)]
async fn upload_file(
    path: &str,
    session_id: &str,
    node: &str,
    bucket_id: Uuid,
    app_id: Uuid,
    token: &str,
    client: &ClientWithMiddleware,
    range: Range<u64>,
    interrupt: bool,
) {
    let file = File::open(path).await.unwrap();
    let size = file.metadata().await.unwrap().len();

    let _ = client
        .put(format!(
            "http://{}/api/file/upload/put/{app_id}/{bucket_id}/{session_id}",
            node,
        ))
        .header(AUTHORIZATION, token.to_string())
        .header(CONTENT_LENGTH, size.to_string())
        .body(if interrupt {
            file_to_body_ranged(file, range).await
        } else {
            file_to_body_ranged_await(file, range).await
        })
        .send()
        .await;
}

pub async fn test_durable_upload(data: (AppDto, BucketDto, String, String)) {
    let (app_dto, bucket_dto, token, _user_token) = data;
    let reqwest_client = reqwest::Client::builder().build().unwrap();
    let client = ClientBuilder::new(reqwest_client).with(Logger).build();

    let size;
    {
        let file = File::open("test_data/test2.txt").await.unwrap();
        size = file.metadata().await.unwrap().len();
    }
    let mid = size / 2;
    let range_a = 0..mid;

    let session = start_upload_session(
        "test_data/test2.txt",
        "test3",
        "127.0.0.2:4000",
        bucket_dto.id,
        app_dto.id,
        &token,
        &client,
    )
    .await;
    upload_file(
        "test_data/test2.txt",
        &session.code,
        "127.0.0.3:4001",
        bucket_dto.id,
        app_dto.id,
        &token,
        &client,
        range_a,
        true,
    )
    .await;

    info!("First half uploaded");

    sleep(Duration::from_secs(2)).await;

    let resume_res = resume_upload_session(
        &session.code,
        "127.0.0.3:4001",
        bucket_dto.id,
        app_dto.id,
        &token,
        &client,
    )
    .await;

    info!("Resuming @ {}", resume_res.uploaded_size);

    upload_file(
        "test_data/test2.txt",
        &session.code,
        "127.0.0.3:4001",
        bucket_dto.id,
        app_dto.id,
        &token,
        &client,
        resume_res.uploaded_size..size,
        false,
    )
    .await;

    download_file(
        "test_data/test3-dl-1.txt",
        "test3",
        "127.0.0.2:4000",
        bucket_dto.id,
        app_dto.id,
        &token,
        &client,
    )
    .await;
    header!("Big Durable File downloaded from remote");

    test_files("test_data/test2.txt", "test_data/test3-dl-1.txt", None).await;
}
