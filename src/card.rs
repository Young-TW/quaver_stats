use axum::{
    extract::{Path, Extension},
    response::{IntoResponse, Response},
};
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use std::io::Cursor;
use image::ImageReader;
use image::imageops::FilterType;
use image::imageops::overlay;
use std::sync::Arc;

use ab_glyph::{FontArc, PxScale};

use crate::user::User;
use crate::avatar::fetch_avatar_from_url;
use crate::cache::Cache;

pub async fn generate_card(
    Path(username): Path<String>,
    Extension(cache): Extension<Arc<Cache>>,
) -> Response {
    // 檢查快取
    if let Some(cached_image) = cache.get(&username).await {
        return (
            [(axum::http::header::CONTENT_TYPE, "image/png")],
            cached_image,
        )
            .into_response();
    }

    // 如果快取不存在，生成卡片
    let card_image = generate_card_image(&username).await;

    // 將生成的卡片存入快取
    cache.set(username.clone(), card_image.clone()).await;

    (
        [(axum::http::header::CONTENT_TYPE, "image/png")],
        card_image,
    )
        .into_response()
}

async fn generate_card_image(username: &str) -> Vec<u8> {
    // 抓取玩家資料
    let user_id = match User::fetch_id(&username).await {
        Ok(u) => u,
        Err(_) => {
            return Vec::new();
        }
    };

    let user_stat = match User::fetch_stat(user_id).await {
        Ok(u) => u,
        Err(_) => {
            return Vec::new();
        }
    };

    // 建立圖卡 256x192，使用背景圖
    let background_path = "assets/image/quaver.jpg";
    let bg_img = ImageReader::open(background_path)
        .expect("無法打開背景圖")
        .decode()
        .expect("無法解析背景圖")
        .resize_exact(256, 192, FilterType::Lanczos3)
        .to_rgba8();

    let mut img = bg_img; // 用背景圖作為畫布

    // 抓取並處理大頭照
    let avatar = fetch_avatar_from_url(&user_stat.avatar_url, (64, 64)).await;
    overlay(&mut img, &avatar.to_rgba8(), 10, 10); // 將大頭照繪製到卡片左上角

    // 載入字型（請確保 assets/JetBrainsMono.ttf 存在）
    let font = FontArc::try_from_slice(include_bytes!("../assets/JetBrainsMono/JetBrainsMono.ttf"))
        .unwrap();
    let scale = PxScale::from(20.0);

    let lines = vec![
        format!("{} ({})", user_stat.name, user_stat.country),
        format!("Global Rank: #{}", user_stat.global_rank),
        format!("Country Rank: #{}", user_stat.country_rank),
        format!("rating: {:.2}", user_stat.rating),
        format!("Accuracy: {:.2}%", user_stat.accuracy),
    ];

    for (i, line) in lines.iter().enumerate() {
        draw_line(&mut img, &line, 10, 80 + i as i32 * 20, scale, &font); // 調整文字位置
    }

    // 輸出成 PNG
    let mut buffer = Cursor::new(Vec::new());
    let dynimg = DynamicImage::ImageRgba8(img);
    dynimg.write_to(&mut buffer, ImageFormat::Png).unwrap();

    buffer.into_inner()
}

// 繪製單行文字
fn draw_line(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    text: &str,
    x: i32,
    y: i32,
    scale: PxScale,
    font: &FontArc,
) {
    use imageproc::drawing::draw_text_mut;
    draw_text_mut(img, Rgba([255, 255, 255, 255]), x, y, scale, font, text);
}
