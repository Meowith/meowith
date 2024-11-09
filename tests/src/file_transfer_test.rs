use crate::header;
use crate::utils::{file_to_body, test_files, Logger};
use commons::permission::PermissionList;
use dashboard_lib::public::auth::auth_routes::{AuthResponse, RegisterRequest};
use dashboard_lib::public::routes::application::CreateApplicationRequest;
use dashboard_lib::public::routes::bucket::CreateBucketRequest;
use dashboard_lib::public::routes::token::AppTokenResponse;
use data::dto::entity::{AppDto, BucketDto, ScopedPermission, TokenIssueRequest};
use data::model::permission_model::UserPermission;
use http::header::{CONTENT_LENGTH, RANGE};
use log::info;
use rand::{distributions::Alphanumeric, Rng};
use reqwest::header::AUTHORIZATION;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use std::ops::Range;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

async fn setup_test_files() -> tokio::io::Result<()> {
    let files: Vec<(&str, usize)> = vec![
        ("test_data/test1.txt", 10_000),
        ("test_data/test2.txt", 1700 * 1024),
    ];
    for (file_name, size) in files {
        let mut file = File::create(file_name).await?;
        let random_letters: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(size)
            .map(char::from)
            .collect();
        file.write_all(random_letters.as_bytes()).await?;
    }
    Ok(())
}
async fn create_user(client: &ClientWithMiddleware) -> String {
    let req = RegisterRequest {
        username: "username".to_string(),
        password: "password".to_string(),
    };

    client
        .post("http://127.0.0.4:4002/api/auth/register")
        .json(&req)
        .send()
        .await
        .expect("")
        .json::<AuthResponse>()
        .await
        .expect("")
        .token
}

async fn create_application(
    user_token: &str,
    name: String,
    client: &ClientWithMiddleware,
) -> AppDto {
    let req = CreateApplicationRequest {
        name,
        quota: 14 * 1024 * 1024 * 1024,
    };

    client
        .post("http://127.0.0.4:4002/api/app/create")
        .json(&req)
        .header(AUTHORIZATION, format!("Bearer {}", user_token))
        .send()
        .await
        .expect("")
        .json::<AppDto>()
        .await
        .expect("")
}

async fn create_bucket(
    user_token: &str,
    app_dto: &AppDto,
    name: String,
    client: &ClientWithMiddleware,
) -> BucketDto {
    let req = CreateBucketRequest {
        name,
        app_id: app_dto.id,
        quota: 256 * 1024 * 1024,
        atomic_upload: false,
    };

    client
        .post("http://127.0.0.4:4002/api/bucket/create")
        .json(&req)
        .header(AUTHORIZATION, format!("Bearer {}", user_token))
        .send()
        .await
        .expect("")
        .json::<BucketDto>()
        .await
        .expect("")
}

async fn fetch_bucket_info(
    user_token: &str,
    app_id: Uuid,
    bucket_id: Uuid,
    client: &ClientWithMiddleware,
) -> BucketDto {
    client
        .get(format!(
            "http://127.0.0.3:4001/api/bucket/info/{app_id}/{bucket_id}"
        ))
        .header(AUTHORIZATION, format!("Bearer {}", user_token))
        .send()
        .await
        .expect("")
        .json::<BucketDto>()
        .await
        .expect("")
}

async fn issue_token(
    app: &AppDto,
    bucket_id: Uuid,
    name: String,
    user_token: &str,
    client: &ClientWithMiddleware,
) -> AppTokenResponse {
    let req = TokenIssueRequest {
        app_id: app.id,
        name,
        perms: vec![ScopedPermission {
            bucket_id,
            allowance: PermissionList(vec![
                UserPermission::Read,
                UserPermission::Write,
                UserPermission::Overwrite,
                UserPermission::ListDirectory,
                UserPermission::ListBucket,
                UserPermission::Rename,
                UserPermission::Delete,
                UserPermission::FetchBucketInfo,
            ])
            .into(),
        }],
    };

    client
        .post("http://127.0.0.4:4002/api/app/token/issue")
        .json(&req)
        .header(AUTHORIZATION, format!("Bearer {}", user_token))
        .send()
        .await
        .expect("")
        .json::<AppTokenResponse>()
        .await
        .expect("")
}

async fn upload_file(
    path: &str,
    remote_path: &str,
    node: &str,
    bucket_id: Uuid,
    app_id: Uuid,
    token: &str,
    client: &ClientWithMiddleware,
) {
    let file = File::open(path).await.unwrap();
    let size = file.metadata().await.unwrap().len();

    client
        .post(format!(
            "http://{}/api/file/upload/oneshot/{}/{}/{}",
            node, app_id, bucket_id, remote_path
        ))
        .header(AUTHORIZATION, token.to_string())
        .header(CONTENT_LENGTH, size.to_string())
        .body(file_to_body(file))
        .send()
        .await
        .expect("");
}

pub async fn download_file(
    path: &str,
    remote_path: &str,
    addr: &str,
    bucket_id: Uuid,
    app_id: Uuid,
    token: &str,
    client: &ClientWithMiddleware,
) {
    let mut response = client
        .get(format!(
            "http://{}/api/file/download/{}/{}/{}",
            addr, app_id, bucket_id, remote_path
        ))
        .header(AUTHORIZATION, token.to_string())
        .send()
        .await
        .expect("");

    let mut file = File::create(path).await.unwrap();
    while let Some(chunk) = response.chunk().await.unwrap() {
        file.write_all(&chunk).await.unwrap()
    }
    file.shutdown().await.unwrap()
}

pub async fn delete_file(
    path: &str,
    addr: &str,
    bucket_id: Uuid,
    app_id: Uuid,
    token: &str,
    client: &ClientWithMiddleware,
) {
    client
        .delete(format!(
            "http://{}/api/file/delete/{app_id}/{bucket_id}/{path}",
            addr
        ))
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .expect("");
}

#[allow(clippy::too_many_arguments)]
async fn download_file_ranged(
    path: &str,
    remote_path: &str,
    addr: &str,
    bucket_id: Uuid,
    app_id: Uuid,
    token: &str,
    client: &ClientWithMiddleware,
    range: Range<u64>,
) {
    let mut response = client
        .get(format!(
            "http://{}/api/file/download/{}/{}/{}",
            addr, app_id, bucket_id, remote_path
        ))
        .header(AUTHORIZATION, token.to_string())
        .header(RANGE, format!("bytes={}-{}", range.start, range.end - 1))
        .send()
        .await
        .expect("");

    let mut file = File::create(path).await.unwrap();
    while let Some(chunk) = response.chunk().await.unwrap() {
        file.write_all(&chunk).await.unwrap()
    }
    file.shutdown().await.unwrap()
}

pub async fn test_file_transfer() -> (AppDto, BucketDto, String, String) {
    setup_test_files().await.unwrap();
    let reqwest_client = reqwest::Client::builder().build().unwrap();
    let client = ClientBuilder::new(reqwest_client).with(Logger).build();

    let user_token = create_user(&client).await;
    info!("Got user_token={user_token}");
    let app_dto = create_application(&user_token, "test".to_string(), &client).await;
    info!("Created application {}", app_dto.name);
    let bucket_dto = create_bucket(&user_token, &app_dto, "test".to_string(), &client).await;
    info!("Created bucket {}", bucket_dto.name);
    let token = issue_token(
        &app_dto,
        bucket_dto.id,
        "test".to_string(),
        &user_token,
        &client,
    )
    .await
    .token;
    header!("Token issued");

    let fetched_bucket_dto: BucketDto =
        fetch_bucket_info(&token, app_dto.id, bucket_dto.id, &client).await;
    assert_eq!(fetched_bucket_dto.space_taken, 0);
    assert_eq!(fetched_bucket_dto.file_count, 0);
    header!("Bucket fetch #1");

    upload_file(
        "test_data/test1.txt",
        "test1",
        "127.0.0.2:4000",
        bucket_dto.id,
        app_dto.id,
        &token,
        &client,
    )
    .await;
    header!("Small File uploaded");

    download_file(
        "test_data/test1-dl-1.txt",
        "test1",
        "127.0.0.2:4000",
        bucket_dto.id,
        app_dto.id,
        &token,
        &client,
    )
    .await;
    header!("Small File downloaded from origin");

    test_files("test_data/test1.txt", "test_data/test1-dl-1.txt", None).await;

    download_file(
        "test_data/test1-dl-2.txt",
        "test1",
        "127.0.0.3:4001",
        bucket_dto.id,
        app_dto.id,
        &token,
        &client,
    )
    .await;
    header!("Small File downloaded from remote");

    test_files("test_data/test1.txt", "test_data/test1-dl-2.txt", None).await;

    upload_file(
        "test_data/test2.txt",
        "test2",
        "127.0.0.3:4001",
        bucket_dto.id,
        app_dto.id,
        &token,
        &client,
    )
    .await;
    header!("Big File uploaded");

    download_file(
        "test_data/test2-dl-1.txt",
        "test2",
        "127.0.0.2:4000",
        bucket_dto.id,
        app_dto.id,
        &token,
        &client,
    )
    .await;
    header!("Big File downloaded from remote");

    test_files("test_data/test2.txt", "test_data/test2-dl-1.txt", None).await;

    download_file(
        "test_data/test2-dl-2.txt",
        "test2",
        "127.0.0.3:4001",
        bucket_dto.id,
        app_dto.id,
        &token,
        &client,
    )
    .await;
    header!("Big File downloaded from origin");

    test_files("test_data/test2.txt", "test_data/test2-dl-2.txt", None).await;

    let range = 1000..1700 * 1024 - 1000;
    download_file_ranged(
        "test_data/test3-dl.txt",
        "test2",
        "127.0.0.3:4001",
        bucket_dto.id,
        app_dto.id,
        &token,
        &client,
        range.clone(),
    )
    .await;
    header!("Big ranged File downloaded");

    test_files(
        "test_data/test2.txt",
        "test_data/test3-dl.txt",
        Some((range.start, range.end)),
    )
    .await;

    let fetched_bucket_dto: BucketDto =
        fetch_bucket_info(&token, app_dto.id, bucket_dto.id, &client).await;
    assert_eq!(fetched_bucket_dto.space_taken, 10_000 + 1700 * 1024);
    assert_eq!(fetched_bucket_dto.file_count, 2);
    header!("Bucket fetch #2");

    delete_file(
        "test1",
        "127.0.0.3:4001",
        bucket_dto.id,
        app_dto.id,
        &token,
        &client,
    )
    .await;
    delete_file(
        "test2",
        "127.0.0.2:4000",
        bucket_dto.id,
        app_dto.id,
        &token,
        &client,
    )
    .await;

    header!("Files deleted");

    (app_dto, bucket_dto, token, user_token)
}
