use crate::file_transfer_test::delete_file;
use crate::utils::Logger;
use data::dto::entity::{AppDto, BucketDto, EntityList, RenameEntityRequest};
use data::dto::entity::{DeleteDirectoryRequest, Entity};
use http::header::{AUTHORIZATION, CONTENT_LENGTH};
use http::StatusCode;
use log::info;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use uuid::Uuid;

pub async fn create_dir(name: &str, args: &NodeArgs<'_>) {
    assert!(args
        .client
        .post(format!(
            "http://{}/api/directory/create/{}/{}/{}",
            args.node, args.app_id, args.bucket_id, name,
        ))
        .header(AUTHORIZATION, args.token)
        .send()
        .await
        .unwrap()
        .status()
        .is_success());
}

pub async fn create_file(name: &str, args: &NodeArgs<'_>) {
    let size = 10;

    assert!(args
        .client
        .post(format!(
            "http://{}/api/file/upload/oneshot/{}/{}/{}",
            args.node, args.app_id, args.bucket_id, name,
        ))
        .header(AUTHORIZATION, args.token.to_string())
        .header(CONTENT_LENGTH, size.to_string())
        .body(vec![0u8; 10])
        .send()
        .await
        .expect("")
        .status()
        .is_success());
}

pub async fn rename_dir(name: &str, new_name: &str, args: &NodeArgs<'_>) {
    let req = RenameEntityRequest {
        to: new_name.to_string(),
    };

    assert!(args
        .client
        .post(format!(
            "http://{}/api/directory/rename/{}/{}/{}",
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

pub async fn list_folder(name: &str, args: &NodeArgs<'_>) -> EntityList {
    args.client
        .get(format!(
            "http://{}/api/directory/list/{}/{}/{}",
            args.node, args.app_id, args.bucket_id, name,
        ))
        .header(AUTHORIZATION, args.token.to_string())
        .send()
        .await
        .unwrap()
        .json::<EntityList>()
        .await
        .unwrap()
}

pub async fn list_bucket_files(args: &NodeArgs<'_>) -> EntityList {
    args.client
        .get(format!(
            "http://{}/api/bucket/list/files/{}/{}",
            args.node, args.app_id, args.bucket_id,
        ))
        .header(AUTHORIZATION, args.token.to_string())
        .send()
        .await
        .unwrap()
        .json::<EntityList>()
        .await
        .unwrap()
}

pub async fn list_bucket_directories(args: &NodeArgs<'_>) -> EntityList {
    args.client
        .get(format!(
            "http://{}/api/bucket/list/directories/{}/{}",
            args.node, args.app_id, args.bucket_id,
        ))
        .header(AUTHORIZATION, args.token.to_string())
        .send()
        .await
        .unwrap()
        .json::<EntityList>()
        .await
        .unwrap()
}

pub async fn delete_dir(name: &str, recurse: bool, args: &NodeArgs<'_>) -> StatusCode {
    let req = DeleteDirectoryRequest { recursive: recurse };

    let resp = args
        .client
        .delete(format!(
            "http://{}/api/directory/delete/{}/{}/{}",
            args.node, args.app_id, args.bucket_id, name
        ))
        .header(AUTHORIZATION, args.token.to_string())
        .json(&req)
        .send()
        .await
        .unwrap();
    let status = resp.status();

    info!("del-dir-res: {}", resp.text().await.unwrap());
    status
}

pub async fn stat_entity(name: &str, args: &NodeArgs<'_>) -> Entity {
    args.client
        .get(format!(
            "http://127.0.0.3:4001/api/bucket/stat/{}/{}/{name}",
            args.app_id, args.bucket_id,
        ))
        .header(AUTHORIZATION, format!("Bearer {}", args.token))
        .send()
        .await
        .expect("")
        .json::<Entity>()
        .await
        .expect("")
}

pub struct NodeArgs<'a> {
    pub node: &'a str,
    pub token: &'a str,
    pub app_id: Uuid,
    pub bucket_id: Uuid,
    pub client: &'a ClientWithMiddleware,
}

macro_rules! assert_contains {
    ($a: expr, $b: expr) => {
        for item in $b {
            let mut found = false;
            for res in &$a {
                if res.name == item {
                    found = true;
                    break;
                }
            }
            assert!(found, "Not found: {item}");
        }
    };
}

pub async fn directory_test(data: (AppDto, BucketDto, String, String)) {
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

    create_dir("test_dir_1", &args).await;
    header!("Created test_dir_1");

    create_file("test1", &args).await;
    header!("created test1");

    create_file("test_dir_1/test1", &args).await;
    header!("test_dir_1/test1");

    create_file("test_dir_2/test1", &args).await;
    header!("created test_dir_2/test1");

    create_dir("test_dir_1/test_dir_3", &args).await;
    header!("Created test_dir_1/test_dir_3");

    create_dir("test_dir_a/test_dir_b/test_dir_4", &args).await;
    header!("Created test_dir_a/test_dir_b/test_dir_4");

    create_file("test_dir_1/test_dir_3/test1", &args).await;
    header!("created test_dir_1/test_dir_3/test1");

    let list = list_folder("", &args).await;
    assert_contains!(
        list.entities,
        ["test1", "test_dir_1", "test_dir_2", "test_dir_a"]
    );

    let list = list_folder("test_dir_1", &args).await;
    assert_contains!(list.entities, ["test1", "test_dir_3",]);

    let list = list_bucket_files(&args).await;
    info!("bucket list: {list:?}");
    assert_contains!(list.entities, ["test1", "test1", "test1", "test1",]);

    let list = list_bucket_directories(&args).await;
    info!("bucket list: {list:?}");
    assert_contains!(
        list.entities,
        [
            "test_dir_1",
            "test_dir_2",
            "test_dir_a",
            "test_dir_a/test_dir_b",
            "test_dir_a/test_dir_b/test_dir_4",
            "test_dir_1/test_dir_3"
        ]
    );

    let test_dir_1 = stat_entity("test_dir_1", &args).await;
    assert_eq!(test_dir_1.name, "test_dir_1");
    assert!(test_dir_1.is_dir);

    let test_dir_b = stat_entity("test_dir_a/test_dir_b", &args).await;
    assert_eq!(test_dir_b.name, "test_dir_b");
    assert!(test_dir_b.is_dir);

    let test_1 = stat_entity("test1", &args).await;
    assert_eq!(test_1.name, "test1");
    assert!(!test_1.is_dir);

    let test_2 = stat_entity("test_dir_1/test_dir_3/test1", &args).await;
    assert_eq!(test_2.name, "test1");
    assert!(!test_2.is_dir);

    header!("Fetched file info");

    rename_dir("test_dir_1", "test_dir_11", &args).await;
    let list = list_folder("test_dir_11", &args).await;
    assert_contains!(list.entities, ["test1", "test_dir_3",]);
    let list = list_folder("", &args).await;
    assert_contains!(
        list.entities,
        ["test1", "test_dir_11", "test_dir_2", "test_dir_a"]
    );

    assert!(delete_dir("test_dir_11", false, &args)
        .await
        .is_client_error());
    delete_file(
        "test_dir_11/test1",
        "127.0.0.2:4000",
        bucket_dto.id,
        app_dto.id,
        &token,
        &client,
    )
    .await;

    delete_file(
        "test_dir_11/test_dir_3/test1",
        "127.0.0.2:4000",
        bucket_dto.id,
        app_dto.id,
        &token,
        &client,
    )
    .await;

    assert!(delete_dir("test_dir_11", false, &args)
        .await
        .is_client_error());
    assert!(delete_dir("test_dir_11", true, &args).await.is_success());
}
