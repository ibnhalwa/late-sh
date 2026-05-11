use anyhow::{Result, bail};
use image::GenericImageView;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

pub async fn fetch_and_render_image(
    url: String,
    max_width: u32,
    max_height: u32,
) -> Result<Vec<Line<'static>>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("late-sh/1.0")
        .build()?;
    fetch_and_render_image_with_client(client, url, max_width, max_height).await
}

pub async fn fetch_and_render_image_with_client(
    client: reqwest::Client,
    url: String,
    max_width: u32,
    max_height: u32,
) -> Result<Vec<Line<'static>>> {
    tracing::info!("Attempting to render inline image: {}", url);
    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        tracing::error!("HTTP error fetching image ({}): {}", url, resp.status());
        bail!("HTTP {}", resp.status());
    }

    let bytes = resp.bytes().await?;
    tracing::info!("Image downloaded: {} bytes", bytes.len());

    tokio::task::spawn_blocking(move || {
        tracing::info!("Decoding image...");
        let img = match image::load_from_memory(&bytes) {
            Ok(img) => img,
            Err(e) => {
                tracing::error!("Image decoding failed: {}", e);
                return Err(e.into());
            }
        };
        tracing::info!("Image decoded: {}x{}", img.width(), img.height());

        let (width, height) = img.dimensions();
        let target_width = width.min(max_width);
        let target_height = height.min(max_height * 2);

        let scale = f32::min(
            target_width as f32 / width as f32,
            target_height as f32 / height as f32,
        );
        let new_w = (width as f32 * scale).round() as u32;
        let new_h = (height as f32 * scale).round() as u32;

        let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::CatmullRom);
        let rgba_img = resized.to_rgba8();
        let (w, h) = rgba_img.dimensions();

        let mut lines = Vec::new();
        for y in (0..h).step_by(2) {
            let mut spans = Vec::new();
            for x in 0..w {
                let top_pixel = rgba_img.get_pixel(x, y);
                let bottom_pixel = if y + 1 < h {
                    rgba_img.get_pixel(x, y + 1)
                } else {
                    &image::Rgba([0, 0, 0, 0])
                };

                let has_fg = top_pixel[3] > 0;
                let has_bg = bottom_pixel[3] > 0;
                if !has_fg && !has_bg {
                    spans.push(Span::raw(" "));
                    continue;
                }

                let mut style = Style::default();
                if has_fg {
                    style = style.fg(Color::Rgb(top_pixel[0], top_pixel[1], top_pixel[2]));
                }
                if has_bg {
                    style = style.bg(Color::Rgb(
                        bottom_pixel[0],
                        bottom_pixel[1],
                        bottom_pixel[2],
                    ));
                }
                spans.push(Span::styled("▀", style));
            }
            lines.push(Line::from(spans));
        }

        Ok::<Vec<Line<'static>>, anyhow::Error>(lines)
    })
    .await?
}
