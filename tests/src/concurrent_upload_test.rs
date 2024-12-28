use crate::directory_test::NodeArgs;
use crate::file_transfer_test::{assert_bucket_info, upload_file, TEST_3_SIZE};
use crate::utils::Logger;
use data::dto::entity::{AppDto, BucketDto};
use reqwest_middleware::ClientBuilder;

pub async fn concurrent_test(data: (AppDto, BucketDto, String, String)) {
    let (app_dto, bucket_dto, token, _user_token) = data;

    let num = 10;
    let mut tasks = vec![];

    for i in 0..num {
        let reqwest_client = reqwest::Client::builder().build().unwrap();
        let client = ClientBuilder::new(reqwest_client).with(Logger).build();
        let token = token.clone();

        tasks.push(tokio::spawn(async move {
            let args = NodeArgs {
                node: "127.0.0.2:4000",
                token: &token,
                user_token: "",
                app_id: app_dto.id,
                bucket_id: bucket_dto.id,
                client: &client,
            };

            upload_file(
                "test_data/test3.txt",
                &format!("test-multiple-{i}"),
                "127.0.0.2:4000",
                &args,
            )
            .await;
        }))
    }

    for task in tasks {
        task.await.unwrap();
    }

    let reqwest_client = reqwest::Client::builder().build().unwrap();
    let client = ClientBuilder::new(reqwest_client).with(Logger).build();
    let token = token.clone();

    let args = NodeArgs {
        node: "127.0.0.2:4000",
        token: &token,
        user_token: "",
        app_id: app_dto.id,
        bucket_id: bucket_dto.id,
        client: &client,
    };

    assert_bucket_info(&args, 10, (10 * TEST_3_SIZE) as i64).await;
}
