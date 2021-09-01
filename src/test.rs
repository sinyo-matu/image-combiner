#[tokio::test(flavor = "multi_thread", worker_threads = 20)]
async fn test_processor() {
    use super::*;
    use rusoto_core::Region;
    use rusoto_s3::{GetObjectRequest, S3Client, S3};
    use tokio::io::AsyncReadExt;
    dotenv::dotenv().unwrap();
    let config = simplelog::ConfigBuilder::new()
        .set_time_format("%F:%T".to_string())
        .add_filter_allow("lambda_generate_bundled_item_image".to_string())
        .set_target_level(simplelog::LevelFilter::Info)
        .build();
    simplelog::SimpleLogger::init(simplelog::LevelFilter::Debug, config).unwrap();
    let s3_client = S3Client::new(Region::ApNortheast1);
    let item_code = "A2103UCS071";
    let image_count = 11;
    let mut image_bytes: Vec<Vec<u8>> = Vec::new();
    for no in 1..=image_count {
        let request = GetObjectRequest {
            bucket: "phitemspics".to_string(),
            key: format!("{}_{}.jpeg", item_code, no),
            ..Default::default()
        };
        let res = s3_client.get_object(request).await.unwrap();
        let res_body = res.body.unwrap();
        let mut image_byte: Vec<u8> = Vec::new();
        res_body
            .into_async_read()
            .read_to_end(&mut image_byte)
            .await
            .unwrap();
        image_bytes.push(image_byte);
    }
    println!("get {} pics", image_bytes.len());
    let processor = ProcessorBuilder::new()
        .set_column(2)
        .set_padding(20)
        .build();
    let image_bytes = processor
        .create_bundled_image_from_bytes(image_bytes)
        .await
        .unwrap();
    // image::load_from_memory(&image_bytes)
    //     .unwrap()
    //     .save(format!("{}_bundled.jpeg", item_code))
    //     .unwrap();
    std::fs::write(format!("{}_bundled.jpeg", item_code), &image_bytes).unwrap();
    // let put_request = rusoto_s3::PutObjectRequest {
    //     bucket: "phitemspics".to_string(),
    //     body: Some(image_bytes.into()),
    //     key: format!("{}_bundled.jpeg", item_code),
    //     ..Default::default()
    // };
    // s3_client.put_object(put_request).await.unwrap();
}
