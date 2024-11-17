use crate::directory_test::NodeArgs;
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

pub const TEST_3_SIZE: usize = 98 * 1024;

async fn setup_test_files() -> tokio::io::Result<()> {
    let files: Vec<(&str, usize)> = vec![
        ("test_data/test1.txt", 10_000),
        ("test_data/test2.txt", 1700 * 1024),
        ("test_data/test3.txt", TEST_3_SIZE),
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

pub(crate) async fn fetch_bucket_info(
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

pub(crate) async fn assert_bucket_info(args: &NodeArgs<'_>, files: i64, space_taken: i64) {
    let fetched_bucket_dto: BucketDto =
        fetch_bucket_info(args.token, args.app_id, args.bucket_id, args.client).await;
    info!("Info assert. Expected f={files} s={space_taken}, got {fetched_bucket_dto:?}");
    assert_eq!(fetched_bucket_dto.space_taken, space_taken);
    assert_eq!(fetched_bucket_dto.file_count, files);
    header!("Bucket fetch");
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

pub(crate) async fn upload_file(path: &str, remote_path: &str, node: &str, args: &NodeArgs<'_>) {
    let file = File::open(path).await.unwrap();
    let size = file.metadata().await.unwrap().len();

    args.client
        .post(format!(
            "http://{}/api/file/upload/oneshot/{}/{}/{}",
            node, args.app_id, args.bucket_id, remote_path
        ))
        .header(AUTHORIZATION, args.token.to_string())
        .header(CONTENT_LENGTH, size.to_string())
        .body(file_to_body(file))
        .send()
        .await
        .expect("");
}

pub async fn download_file(path: &str, remote_path: &str, addr: &str, args: &NodeArgs<'_>) {
    let mut response = args
        .client
        .get(format!(
            "http://{}/api/file/download/{}/{}/{}",
            addr, args.app_id, args.bucket_id, remote_path
        ))
        .header(AUTHORIZATION, args.token.to_string())
        .send()
        .await
        .expect("");

    let mut file = File::create(path).await.unwrap();
    while let Some(chunk) = response.chunk().await.unwrap() {
        file.write_all(&chunk).await.unwrap()
    }
    file.shutdown().await.unwrap()
}

pub async fn delete_file(path: &str, addr: &str, args: &NodeArgs<'_>) {
    args.client
        .delete(format!(
            "http://{}/api/file/delete/{}/{}/{path}",
            addr, args.app_id, args.bucket_id,
        ))
        .header(AUTHORIZATION, format!("Bearer {}", args.token))
        .send()
        .await
        .expect("");
}

#[allow(clippy::too_many_arguments)]
async fn download_file_ranged(
    path: &str,
    remote_path: &str,
    addr: &str,
    args: &NodeArgs<'_>,
    range: Range<u64>,
) {
    let mut response = args
        .client
        .get(format!(
            "http://{}/api/file/download/{}/{}/{}",
            addr, args.app_id, args.bucket_id, remote_path
        ))
        .header(AUTHORIZATION, args.token.to_string())
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

    let args = NodeArgs {
        node: "127.0.0.2:4000",
        token: &token,
        app_id: app_dto.id,
        bucket_id: bucket_dto.id,
        client: &client,
    };

    assert_bucket_info(&args, 0, 0).await;

    upload_file("test_data/test1.txt", "test1", "127.0.0.2:4000", &args).await;
    header!("Small File uploaded");

    download_file("test_data/test1-dl-1.txt", "test1", "127.0.0.2:4000", &args).await;
    header!("Small File downloaded from origin");

    test_files("test_data/test1.txt", "test_data/test1-dl-1.txt", None).await;

    download_file("test_data/test1-dl-2.txt", "test1", "127.0.0.3:4001", &args).await;
    header!("Small File downloaded from remote");

    test_files("test_data/test1.txt", "test_data/test1-dl-2.txt", None).await;

    upload_file("test_data/test2.txt", "test2", "127.0.0.3:4001", &args).await;
    header!("Big File uploaded");

    download_file("test_data/test2-dl-1.txt", "test2", "127.0.0.2:4000", &args).await;
    header!("Big File downloaded from remote");

    test_files("test_data/test2.txt", "test_data/test2-dl-1.txt", None).await;

    download_file("test_data/test2-dl-2.txt", "test2", "127.0.0.3:4001", &args).await;
    header!("Big File downloaded from origin");

    test_files("test_data/test2.txt", "test_data/test2-dl-2.txt", None).await;

    let range = 1000..1700 * 1024 - 1000;
    download_file_ranged(
        "test_data/test3-dl.txt",
        "test2",
        "127.0.0.3:4001",
        &args,
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

    assert_bucket_info(&args, 2, 10_000 + 1700 * 1024).await;

    upload_file("test_data/test1.txt", "test1", "127.0.0.2:4000", &args).await;
    header!("Small File uploaded");

    assert_bucket_info(&args, 2, 10_000 + 1700 * 1024).await;

    delete_file("test1", "127.0.0.3:4001", &args).await;
    delete_file("test2", "127.0.0.2:4000", &args).await;

    header!("Files deleted");

    assert_bucket_info(&args, 0, 0).await;

    (app_dto, bucket_dto, token, user_token)
}
