use crate::directory_test::{create_dir, create_file, stat_entity, NodeArgs, FILE_SIZE};
use crate::file_transfer_test::{assert_bucket_info, delete_file};
use crate::utils::Logger;
use data::dto::entity::{AppDto, BucketDto, RenameEntityRequest};
use http::header::AUTHORIZATION;
use log::info;
use reqwest_middleware::ClientBuilder;

pub async fn rename_file(name: &str, new_name: &str, args: &NodeArgs<'_>) {
    let req = RenameEntityRequest {
        to: new_name.to_string(),
    };

    assert!(args
        .client
        .post(format!(
            "http://{}/api/file/rename/{}/{}/{}",
            args.node, args.app_id, args.bucket_id, name,
        ))
        .header(AUTHORIZATION, args.token.to_string())
        .json(&req)
        .send()
        .await
        .expect("")
        .status()
        .is_success());
}

pub async fn move_test(data: (AppDto, BucketDto, String, String)) {
    let (app_dto, bucket_dto, token, _user_token) = data;
    let reqwest_client = reqwest::Client::builder().build().unwrap();
    let client = ClientBuilder::new(reqwest_client).with(Logger).build();

    let args = NodeArgs {
        node: "127.0.0.2:4000",
        token: &token,
        app_id: app_dto.id,
        bucket_id: bucket_dto.id,
        client: &client,
    };

    create_file("test1", &args).await;
    create_dir("test_dir_1", &args).await;
    create_file("test_dir_2/test1", &args).await;
    create_dir("test_dir_1/test_dir_3", &args).await;
    header!("Created test files");
    assert_bucket_info(&args, 2, (2 * FILE_SIZE) as i64).await;

    assert_eq!(stat_entity("test1", &args).await.name, "test1");
    rename_file("test1", "test2", &args).await;
    assert_eq!(stat_entity("test2", &args).await.name, "test2");
    assert_bucket_info(&args, 2, (2 * FILE_SIZE) as i64).await;

    rename_file("test2", "test_dir_1/test2", &args).await;
    assert_eq!(stat_entity("test_dir_1/test2", &args).await.name, "test2");
    assert_bucket_info(&args, 2, (2 * FILE_SIZE) as i64).await;

    assert_eq!(stat_entity("test_dir_2/test1", &args).await.name, "test1"); // old file
    rename_file("test_dir_1/test2", "test_dir_2/test1", &args).await;
    assert_eq!(stat_entity("test_dir_2/test1", &args).await.name, "test1"); // overwritten file
    assert_bucket_info(&args, 1, FILE_SIZE as i64).await;

    rename_file("test_dir_2/test1", "test_dir_1/test_dir_3/test1", &args).await;
    assert_eq!(
        stat_entity("test_dir_1/test_dir_3/test1", &args).await.name,
        "test1"
    );
    assert_bucket_info(&args, 1, FILE_SIZE as i64).await;

    delete_file("test_dir_1/test_dir_3/test1", args.node, &args).await;
}
