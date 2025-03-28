use axum::{
    extract::Path,
    response::{IntoResponse, Response},
};
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use std::io::Cursor;

use ab_glyph::{FontArc, PxScale};

use crate::user::User;

pub async fn generate_card(Path(username): Path<String>) -> Response {
    // 抓取玩家資料
    let user_id = match User::fetch_id(&username).await {
        Ok(u) => u,
        Err(_) => {
            return Response::builder()
                .status(404)
                .body("User not found".into())
                .unwrap();
        }
    };

    let user_stat = match User::fetch_stat(user_id).await {
        Ok(u) => u,
        Err(_) => {
            return Response::builder()
                .status(404)
                .body("User not found".into())
                .unwrap();
        }
    };

    // 建立圖卡 256x192
    let mut img = ImageBuffer::from_pixel(256, 192, Rgba([255, 255, 255, 255]));

    // 載入字型（請確保 assets/JetBrainsMono.ttf 存在）
    let font = FontArc::try_from_slice(include_bytes!("../assets/JetBrainsMono/JetBrainsMono.ttf"))
        .unwrap();
    let scale = PxScale::from(16.0);

    let lines = vec![
        format!("{} ({})", user_stat.name, user_stat.country),
        format!("Global Rank: #{}", user_stat.global_rank),
        format!("Country Rank: #{}", user_stat.country_rank),
        format!("PP: {:.2}", user_stat.rating),
        format!("Accuracy: {:.2}%", user_stat.accuracy),
    ];

    for (i, line) in lines.iter().enumerate() {
        draw_line(&mut img, &line, 10, 10 + i as i32 * 20, scale, &font);
    }

    // 輸出成 PNG
    let mut buffer = Cursor::new(Vec::new());
    let dynimg = DynamicImage::ImageRgba8(img);
    dynimg.write_to(&mut buffer, ImageFormat::Png).unwrap();

    (
        [(axum::http::header::CONTENT_TYPE, "image/png")],
        buffer.into_inner(),
    )
        .into_response()
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
    draw_text_mut(img, Rgba([0, 0, 0, 255]), x, y, scale, font, text);
}
