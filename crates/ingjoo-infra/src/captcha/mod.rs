use anyhow::{anyhow, Result};

pub struct CaptchaGenerator;

impl CaptchaGenerator {
    pub fn generate() -> Result<(String, String)> {
        let chars = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        let mut rng: rand::rngs::StdRng = rand::SeedableRng::from_rng(rand::thread_rng())
            .map_err(|e| anyhow!("初始化随机数生成器失败: {}", e))?;
        let answer: String = (0..4)
            .map(|_| chars.chars().nth(rand::Rng::gen_range(&mut rng, 0..chars.len())).unwrap())
            .collect();

        let width = 120u32;
        let height = 40u32;
        let mut img = image::RgbImage::from_pixel(width, height, image::Rgb([240u8, 240u8, 240u8]));

        for _ in 0..6 {
            let x1 = rand::Rng::gen_range(&mut rng, 0..width);
            let y1 = rand::Rng::gen_range(&mut rng, 0..height);
            let x2 = rand::Rng::gen_range(&mut rng, 0..width);
            let y2 = rand::Rng::gen_range(&mut rng, 0..height);
            let color = image::Rgb([
                rand::Rng::gen_range(&mut rng, 100u8..200),
                rand::Rng::gen_range(&mut rng, 100u8..200),
                rand::Rng::gen_range(&mut rng, 100u8..200),
            ]);
            let dx = (x2 as i32).saturating_sub(x1 as i32);
            let dy = (y2 as i32).saturating_sub(y1 as i32);
            let steps = std::cmp::max(dx.abs(), dy.abs()).max(1);
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let px = (x1 as i32 + (dx as f32 * t) as i32) as u32;
                let py = (y1 as i32 + (dy as f32 * t) as i32) as u32;
                if px < width && py < height {
                    img.put_pixel(px, py, color);
                }
            }
        }

        let simple_font: std::collections::HashMap<char, [[bool; 5]; 7]> = {
            let mut f = std::collections::HashMap::new();
            let patterns: &[(char, &[[bool; 5]; 7])] = &[
                ('A', &[[false,true,true,true,false],[true,false,false,false,true],[true,false,false,false,true],[true,true,true,true,true],[true,false,false,false,true],[true,false,false,false,true],[true,false,false,false,true]]),
                ('B', &[[true,true,true,true,false],[true,false,false,false,true],[true,false,false,false,true],[true,true,true,true,false],[true,false,false,false,true],[true,false,false,false,true],[true,true,true,true,false]]),
                ('C', &[[false,true,true,true,false],[true,false,false,false,true],[true,false,false,false,false],[true,false,false,false,false],[true,false,false,false,false],[true,false,false,false,true],[false,true,true,true,false]]),
                ('D', &[[true,true,true,true,false],[true,false,false,false,true],[true,false,false,false,true],[true,false,false,false,true],[true,false,false,false,true],[true,false,false,false,true],[true,true,true,true,false]]),
                ('E', &[[true,true,true,true,true],[true,false,false,false,false],[true,false,false,false,false],[true,true,true,true,false],[true,false,false,false,false],[true,false,false,false,false],[true,true,true,true,true]]),
                ('F', &[[true,true,true,true,true],[true,false,false,false,false],[true,false,false,false,false],[true,true,true,true,false],[true,false,false,false,false],[true,false,false,false,false],[true,false,false,false,false]]),
                ('G', &[[false,true,true,true,false],[true,false,false,false,true],[true,false,false,false,false],[true,false,true,true,true],[true,false,false,false,true],[true,false,false,false,true],[false,true,true,true,false]]),
                ('H', &[[true,false,false,false,true],[true,false,false,false,true],[true,false,false,false,true],[true,true,true,true,true],[true,false,false,false,true],[true,false,false,false,true],[true,false,false,false,true]]),
                ('J', &[[false,false,true,true,true],[false,false,false,false,true],[false,false,false,false,true],[false,false,false,false,true],[false,false,false,false,true],[true,false,false,false,true],[false,true,true,true,false]]),
                ('K', &[[true,false,false,false,true],[true,false,false,true,false],[true,false,true,false,false],[true,true,false,false,false],[true,false,true,false,false],[true,false,false,true,false],[true,false,false,false,true]]),
                ('L', &[[true,false,false,false,false],[true,false,false,false,false],[true,false,false,false,false],[true,false,false,false,false],[true,false,false,false,false],[true,false,false,false,false],[true,true,true,true,true]]),
                ('M', &[[true,false,false,false,true],[true,true,false,true,true],[true,false,true,false,true],[true,false,true,false,true],[true,false,false,false,true],[true,false,false,false,true],[true,false,false,false,true]]),
                ('N', &[[true,false,false,false,true],[true,true,false,false,true],[true,false,true,false,true],[true,false,false,true,true],[true,false,false,false,true],[true,false,false,false,true],[true,false,false,false,true]]),
                ('P', &[[true,true,true,true,false],[true,false,false,false,true],[true,false,false,false,true],[true,true,true,true,false],[true,false,false,false,false],[true,false,false,false,false],[true,false,false,false,false]]),
                ('U', &[[true,false,false,false,true],[true,false,false,false,true],[true,false,false,false,true],[true,false,false,false,true],[true,false,false,false,true],[true,false,false,false,true],[false,true,true,true,false]]),
                ('V', &[[true,false,false,false,true],[true,false,false,false,true],[true,false,false,false,true],[false,true,false,true,false],[false,true,false,true,false],[false,false,true,false,false],[false,false,true,false,false]]),
                ('W', &[[true,false,false,false,true],[true,false,false,false,true],[true,false,false,false,true],[true,false,true,false,true],[true,false,true,false,true],[true,true,false,true,true],[true,false,false,false,true]]),
                ('X', &[[true,false,false,false,true],[false,true,false,true,false],[false,false,true,false,false],[false,true,false,true,false],[true,false,false,false,true],[true,false,false,false,true],[true,false,false,false,true]]),
                ('Y', &[[true,false,false,false,true],[false,true,false,true,false],[false,false,true,false,false],[false,false,true,false,false],[false,false,true,false,false],[false,false,true,false,false],[false,false,true,false,false]]),
                ('Z', &[[true,true,true,true,true],[false,false,false,true,false],[false,false,true,false,false],[false,true,false,false,false],[true,false,false,false,false],[true,false,false,false,false],[true,true,true,true,true]]),
                ('2', &[[false,true,true,true,false],[true,false,false,false,true],[false,false,false,false,true],[false,false,false,true,false],[false,true,false,false,false],[true,false,false,false,false],[true,true,true,true,true]]),
                ('3', &[[false,true,true,true,false],[true,false,false,false,true],[false,false,false,false,true],[false,false,true,true,false],[false,false,false,false,true],[true,false,false,false,true],[false,true,true,true,false]]),
                ('4', &[[true,false,false,true,false],[true,false,false,true,false],[true,false,false,true,false],[true,true,true,true,true],[false,false,false,true,false],[false,false,false,true,false],[false,false,false,true,false]]),
                ('5', &[[true,true,true,true,true],[true,false,false,false,false],[true,false,false,false,false],[true,true,true,true,false],[false,false,false,false,true],[false,false,false,false,true],[true,true,true,true,false]]),
                ('6', &[[false,true,true,true,false],[true,false,false,false,false],[true,false,false,false,false],[true,true,true,true,false],[true,false,false,false,true],[true,false,false,false,true],[false,true,true,true,false]]),
                ('7', &[[true,true,true,true,true],[false,false,false,false,true],[false,false,false,true,false],[false,false,true,false,false],[false,false,true,false,false],[false,true,false,false,false],[false,true,false,false,false]]),
                ('8', &[[false,true,true,true,false],[true,false,false,false,true],[true,false,false,false,true],[false,true,true,true,false],[true,false,false,false,true],[true,false,false,false,true],[false,true,true,true,false]]),
                ('9', &[[false,true,true,true,false],[true,false,false,false,true],[true,false,false,false,true],[false,true,true,true,true],[false,false,false,false,true],[false,false,false,false,true],[false,true,true,true,false]]),
            ];
            for &(ch, pattern) in patterns {
                f.insert(ch, *pattern);
            }
            f
        };

        let cell_w = 24u32;
        let _cell_h = 32u32;
        let start_x = 6u32;
        let start_y = 4u32;
        for (i, ch) in answer.chars().enumerate() {
            let pattern = simple_font.get(&ch);
            let offset_x = rand::Rng::gen_range(&mut rng, 0u32..3);
            let offset_y = rand::Rng::gen_range(&mut rng, 0u32..4);
            let color = image::Rgb([
                rand::Rng::gen_range(&mut rng, 0u8..100),
                rand::Rng::gen_range(&mut rng, 0u8..100),
                rand::Rng::gen_range(&mut rng, 100u8..200),
            ]);
            if let Some(pattern) = pattern {
                for (row, line) in pattern.iter().enumerate() {
                    for (col, &on) in line.iter().enumerate() {
                        if on {
                            let px = start_x + (i as u32) * cell_w + col as u32 + offset_x;
                            let py = start_y + row as u32 + offset_y;
                            if px < width && py < height {
                                img.put_pixel(px, py, color);
                            }
                        }
                    }
                }
            }
        }

        for _ in 0..60 {
            let x = rand::Rng::gen_range(&mut rng, 0..width);
            let y = rand::Rng::gen_range(&mut rng, 0..height);
            let color = image::Rgb([
                rand::Rng::gen_range(&mut rng, 100u8..200),
                rand::Rng::gen_range(&mut rng, 100u8..200),
                rand::Rng::gen_range(&mut rng, 100u8..200),
            ]);
            img.put_pixel(x, y, color);
        }

        let mut png_buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png_buf, image::ImageFormat::Png)
            .map_err(|e| anyhow!("生成验证码图片失败: {}", e))?;
        let base64_img = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png_buf.into_inner());
        let data_uri = format!("data:image/png;base64,{}", base64_img);

        Ok((answer, data_uri))
    }

    pub fn verify(answer: &str, input: &str) -> bool {
        answer.to_uppercase() == input.to_uppercase()
    }
}
