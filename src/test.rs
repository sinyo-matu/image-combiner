#[tokio::test(flavor = "multi_thread", worker_threads = 20)]
async fn test_processor() {
    use super::*;
    use rusoto_core::Region;
    use rusoto_s3::{GetObjectRequest, S3Client, S3};
    use tokio::io::AsyncReadExt;
    dotenv::dotenv().unwrap();
    let config = simplelog::ConfigBuilder::new()
        .set_time_format("%F:%T".to_string())
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
    let option = CreateBundledImageOptionsBuilder::new()
        .set_column(2)
        .set_padding(20)
        .build();
    let image_bytes = create_bundled_image_from_bytes(image_bytes, option)
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

#[tokio::test(flavor = "multi_thread", worker_threads = 20)]
async fn text_add_table() {
    use super::*;
    let config = simplelog::ConfigBuilder::new()
        .set_time_format("%F:%T".to_string())
        .build();
    simplelog::SimpleLogger::init(simplelog::LevelFilter::Debug, config).unwrap();
    let origin_image = std::fs::read("./test/A2113PE_225_bundled_column2.jpeg").unwrap();
    let head = vec![
        "SIZE".to_string(),
        "??????".to_string(),
        "??????".to_string(),
        "??????".to_string(),
        "??????".to_string(),
        "??????".to_string(),
    ];
    let row1 = vec![
        "M".to_string(),
        "88".to_string(),
        "88".to_string(),
        "88".to_string(),
        "88".to_string(),
        "88".to_string(),
    ];
    let row2 = vec![
        "M".to_string(),
        "88".to_string(),
        "88".to_string(),
        "88".to_string(),
        "88".to_string(),
        "88".to_string(),
    ];
    let body = vec![row1, row2];
    let table = TableBase::new(head, body, 2).unwrap();
    let font_bytes = std::fs::read("./test/TaipeiSansTCBeta-Light.ttf").unwrap();
    let new_image = add_table(origin_image, table, &font_bytes).await.unwrap();
    std::fs::write("./test/add_table.jpeg", &new_image).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 20)]
async fn test_create_bundle_with_table() {
    use super::*;
    use rusoto_core::Region;
    use rusoto_s3::{GetObjectRequest, S3Client, S3};
    use tokio::io::AsyncReadExt;
    dotenv::dotenv().unwrap();
    let config = simplelog::ConfigBuilder::new()
        .set_time_format("%F:%T".to_string())
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
    let option = CreateBundledImageOptionsBuilder::new()
        .set_column(2)
        .set_padding(20)
        .build();
    let head = vec![
        "SIZE".to_string(),
        "??????".to_string(),
        "??????".to_string(),
        "??????".to_string(),
        "??????".to_string(),
        "??????".to_string(),
    ];
    let row1 = vec![
        "M".to_string(),
        "88".to_string(),
        "88".to_string(),
        "88".to_string(),
        "88".to_string(),
        "88".to_string(),
    ];
    let row2 = vec![
        "M".to_string(),
        "88".to_string(),
        "88".to_string(),
        "88".to_string(),
        "88".to_string(),
        "88".to_string(),
    ];
    let body = vec![row1, row2];
    let table = TableBase::new(head, body, 2).unwrap();
    let font_bytes = std::fs::read("./test/TaipeiSansTCBeta-Light.ttf").unwrap();
    let image_bytes =
        create_bundled_image_from_bytes_with_table(image_bytes, table, option, &font_bytes)
            .await
            .unwrap();
    // image::load_from_memory(&image_bytes)
    //     .unwrap()
    //     .save(format!("{}_bundled.jpeg", item_code))
    //     .unwrap();
    std::fs::write(format!("./test/{}_bundled.jpeg", item_code), &image_bytes).unwrap();
    // let put_request = rusoto_s3::PutObjectRequest {
    //     bucket: "phitemspics".to_string(),
    //     body: Some(image_bytes.into()),
    //     key: format!("{}_bundled.jpeg", item_code),
    //     ..Default::default()
    // };
    // s3_client.put_object(put_request).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 20)]
async fn test_create_bundle_with_text() {
    use super::*;
    use rusoto_core::Region;
    use rusoto_s3::{GetObjectRequest, S3Client, S3};
    use tokio::io::AsyncReadExt;
    dotenv::dotenv().unwrap();
    let config = simplelog::ConfigBuilder::new()
        .set_time_format("%F:%T".to_string())
        .set_target_level(simplelog::LevelFilter::Info)
        .build();
    simplelog::SimpleLogger::init(simplelog::LevelFilter::Debug, config).unwrap();
    let s3_client = S3Client::new(Region::ApNortheast1);
    let item_code = "A2113FB_168";
    let image_count = 16;
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
    let option = CreateBundledImageOptionsBuilder::new()
        .set_column(2)
        .set_padding(20)
        .build();
    let font_bytes = std::fs::read("./test/TaipeiSansTCBeta-Light.ttf").unwrap();
    let image_bytes = create_bundled_image_from_bytes_with_text(
        image_bytes,
        &"???60.0?????????42.0?????????52.5?????????26.5?????????16.0".replace("???", " "),
        option,
        &font_bytes,
    )
    .await
    .unwrap();
    // image::load_from_memory(&image_bytes)
    //     .unwrap()
    //     .save(format!("{}_bundled.jpeg", item_code))
    //     .unwrap();
    std::fs::write(format!("./test/{}_bundled.jpeg", item_code), &image_bytes).unwrap();
    // let put_request = rusoto_s3::PutObjectRequest {
    //     bucket: "phitemspics".to_string(),
    //     body: Some(image_bytes.into()),
    //     key: format!("{}_bundled.jpeg", item_code),
    //     ..Default::default()
    // };
    // s3_client.put_object(put_request).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 20)]
async fn text_create_table_image() {
    use super::*;
    let head = vec![
        "??????".to_string(),
        "??????".to_string(),
        "??????".to_string(),
        "??????".to_string(),
    ];
    let row1 = vec![
        "FREE".to_string(),
        "58".to_string(),
        "78".to_string(),
        "65.5".to_string(),
    ];
    let body = vec![row1];
    let table = TableBase::new(head, body, 2).unwrap();
    let font_bytes = std::fs::read("./test/TaipeiSansTCBeta-Light.ttf").unwrap();
    let image_bytes = create_table_image(table, &font_bytes).await.unwrap();

    std::fs::write("./test/table.jpg", &image_bytes).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 20)]
async fn test_create_text_image() {
    use super::*;
    let font_bytes = std::fs::read("./test/TaipeiSansTCBeta-Light.ttf").unwrap();
    let image_bytes = create_text_image(
        &"???122.0?????????38.0?????????57.0?????????30.0?????????16.0???????????????".replace("???", " "),
        &font_bytes,
    )
    .await
    .unwrap();
    // image::load_from_memory(&image_bytes)
    //     .unwrap()
    //     .save(format!("{}_bundled.jpeg", item_code))
    //     .unwrap();
    std::fs::write("./test/text_image.jpg", &image_bytes).unwrap();
    // let put_request = rusoto_s3::PutObjectRequest {
    //     bucket: "phitemspics".to_string(),
    //     body: Some(image_bytes.into()),
    //     key: format!("{}_bundled.jpeg", item_code),
    //     ..Default::default()
    // };
    // s3_client.put_object(put_request).await.unwrap();
}
