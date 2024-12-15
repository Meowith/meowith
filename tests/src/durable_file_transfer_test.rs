use crate::directory_test::NodeArgs;
use crate::file_transfer_test::{assert_bucket_info, delete_file, download_file};
use crate::utils::{file_to_body_ranged, file_to_body_ranged_await, test_files, Logger};
use data::dto::entity::{
    AppDto, BucketDto, UploadSessionRequest, UploadSessionResumeRequest,
    UploadSessionResumeResponse, UploadSessionStartResponse,
};
use http::header::{AUTHORIZATION, CONTENT_LENGTH};
use log::info;
use reqwest_middleware::ClientBuilder;
use std::ops::Range;
use std::time::Duration;
use tokio::fs::File;
use tokio::time::sleep;

async fn start_upload_session(
    path: &str,
    remote_path: &str,
    node: &str,
    args: &NodeArgs<'_>,
) -> UploadSessionStartResponse {
    let file = File::open(path).await.unwrap();
    let size = file.metadata().await.unwrap().len();
    let req = UploadSessionRequest { size };

    args.client
        .post(format!(
            "http://{node}/api/file/upload/durable/{}/{}/{remote_path}",
            args.app_id, args.bucket_id
        ))
        .json(&req)
        .header(AUTHORIZATION, format!("Bearer {}", args.token))
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
    args: &NodeArgs<'_>,
) -> UploadSessionResumeResponse {
    let req = UploadSessionResumeRequest {
        session_id: session_id.to_string().parse().unwrap(),
    };

    args.client
        .post(format!(
            "http://{node}/api/file/upload/resume/{}/{}",
            args.app_id, args.bucket_id
        ))
        .json(&req)
        .header(AUTHORIZATION, format!("Bearer {}", args.token))
        .send()
        .await
        .expect("")
        .json::<UploadSessionResumeResponse>()
        .await
        .expect("")
}

async fn upload_file(
    path: &str,
    session_id: &str,
    node: &str,
    args: &NodeArgs<'_>,
    range: Range<u64>,
    interrupt: bool,
) {
    let file = File::open(path).await.unwrap();
    let size = file.metadata().await.unwrap().len();

    let _ = args
        .client
        .put(format!(
            "http://{}/api/file/upload/put/{}/{}/{session_id}",
            node, args.app_id, args.bucket_id,
        ))
        .header(AUTHORIZATION, args.token.to_string())
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

    let args = NodeArgs {
        node: "127.0.0.2:4000",
        token: &token,
        app_id: app_dto.id,
        bucket_id: bucket_dto.id,
        client: &client,
    };

    assert_bucket_info(&args, 0, 0).await;

    let session =
        start_upload_session("test_data/test2.txt", "test3", "127.0.0.2:4000", &args).await;
    upload_file(
        "test_data/test2.txt",
        &session.code,
        "127.0.0.3:4001",
        &args,
        range_a,
        true,
    )
    .await;

    info!("First half uploaded");

    assert_bucket_info(&args, 0, 0).await;

    sleep(Duration::from_secs(2)).await;

    let resume_res = resume_upload_session(&session.code, "127.0.0.3:4001", &args).await;

    info!("Resuming @ {}", resume_res.uploaded_size);

    upload_file(
        "test_data/test2.txt",
        &session.code,
        "127.0.0.3:4001",
        &args,
        resume_res.uploaded_size..size,
        false,
    )
    .await;

    download_file("test_data/test3-dl-1.txt", "test3", "127.0.0.2:4000", &args).await;
    header!("Big Durable File downloaded from remote");

    test_files("test_data/test2.txt", "test_data/test3-dl-1.txt", None).await;

    assert_bucket_info(&args, 1, size as i64).await;

    let session =
        start_upload_session("test_data/test2.txt", "test3", "127.0.0.2:4000", &args).await;
    upload_file(
        "test_data/test2.txt",
        &session.code,
        "127.0.0.3:4001",
        &args,
        0..size,
        true,
    )
    .await;

    download_file("test_data/test3-dl-1.txt", "test3", "127.0.0.2:4000", &args).await;
    header!("Big Durable File downloaded from remote");

    test_files("test_data/test2.txt", "test_data/test3-dl-1.txt", None).await;

    assert_bucket_info(&args, 1, size as i64).await;

    delete_file("test3", "127.0.0.2:4000", &args).await;

    assert_bucket_info(&args, 0, 0).await;

    let third = size / 3;
    let two_thirds = size * 2 / 3;
    let range_a = 0..third;

    let session =
        start_upload_session("test_data/test2.txt", "test3", "127.0.0.2:4000", &args).await;
    upload_file(
        "test_data/test2.txt",
        &session.code,
        "127.0.0.3:4001",
        &args,
        range_a,
        true,
    )
    .await;
    info!("First third uploaded");

    sleep(Duration::from_secs(1)).await;
    let resume_res = resume_upload_session(&session.code, "127.0.0.2:4000", &args).await;
    assert!(resume_res.uploaded_size <= third);
    assert!(resume_res.uploaded_size > 0);
    info!("Resuming @ {}", resume_res.uploaded_size);

    upload_file(
        "test_data/test2.txt",
        &session.code,
        "127.0.0.2:4000",
        &args,
        resume_res.uploaded_size..two_thirds,
        true,
    )
    .await;
    info!("Second third uploaded");

    sleep(Duration::from_secs(1)).await;
    let resume_res = resume_upload_session(&session.code, "127.0.0.2:4000", &args).await;
    info!("Resuming @ {}", resume_res.uploaded_size);
    assert!(resume_res.uploaded_size <= two_thirds);

    upload_file(
        "test_data/test2.txt",
        &session.code,
        "127.0.0.2:4000",
        &args,
        resume_res.uploaded_size..size,
        false,
    )
    .await;

    download_file("test_data/test3-dl-2.txt", "test3", "127.0.0.2:4000", &args).await;
    header!("Big Durable File 3 chunked downloaded from remote");
    test_files("test_data/test2.txt", "test_data/test3-dl-2.txt", None).await;

    delete_file("test3", "127.0.0.2:4000", &args).await;

    assert_bucket_info(&args, 0, 0).await;
}
