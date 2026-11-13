use image::RgbaImage;

pub const SPRITE_W: u32 = 192;
pub const SPRITE_H: u32 = 288;
pub const ATLAS_COLS: u32 = 5;
pub const ATLAS_ROWS: u32 = 4;
pub const ATLAS_W: u32 = SPRITE_W * ATLAS_COLS; // 960
pub const ATLAS_H: u32 = SPRITE_H * ATLAS_ROWS; // 1152

pub const FRAME_NAMES: [&str; 20] = [
    "Idle Front",
    "Idle Diag",
    "Idle Side",
    "Idle Back",
    "Walk Front A",
    "Walk Front B",
    "Walk Diag A",
    "Walk Diag B",
    "Walk Side A",
    "Walk Side B",
    "Aim Telegraph",
    "Shoot",
    "Recoil",
    "Reload",
    "Cover Crouch",
    "Alert",
    "Hit Flinch",
    "Hit Leg Kneel",
    "Down Falling",
    "Downed",
];

pub struct GameAssets {
    pub unit_atlas: RgbaImage,
    pub wall_tex: RgbaImage,
    pub cover_tex: RgbaImage,
    pub weapon_tex: RgbaImage,
}

fn remove_white_bg_floodfill(img: &mut RgbaImage, threshold: u8) {
    let (w, h) = img.dimensions();
    let total = (w * h) as usize;
    let mut visited = vec![false; total];
    let mut stack = Vec::with_capacity((w + h) as usize * 2);

    for x in 0..w {
        stack.push((x, 0));
        stack.push((x, h - 1));
    }
    for y in 1..h.saturating_sub(1) {
        stack.push((0, y));
        stack.push((w - 1, y));
    }

    while let Some((x, y)) = stack.pop() {
        let idx = (y * w + x) as usize;
        if visited[idx] {
            continue;
        }
        visited[idx] = true;

        let p = img.get_pixel_mut(x, y);
        if p[0] >= threshold && p[1] >= threshold && p[2] >= threshold {
            p[3] = 0;
            if x > 0 {
                stack.push((x - 1, y));
            }
            if x + 1 < w {
                stack.push((x + 1, y));
            }
            if y > 0 {
                stack.push((x, y - 1));
            }
            if y + 1 < h {
                stack.push((x, y + 1));
            }
        }
    }

    // Soft 1px edge feathering
    let raw = img.as_raw().clone();
    let stride = (w * 4) as usize;
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let off = (y * w + x) as usize * 4;
            if raw[off + 3] == 0 {
                continue;
            }
            if raw[off] > 210 && raw[off + 1] > 210 && raw[off + 2] > 210 {
                let adj_trans = raw[off - 4 + 3] == 0
                    || raw[off + 4 + 3] == 0
                    || raw[off - stride + 3] == 0
                    || raw[off + stride + 3] == 0;
                if adj_trans {
                    img.get_pixel_mut(x, y)[3] = 96;
                }
            }
        }
    }
}

fn opaque_bounds(img: &RgbaImage) -> (u32, u32, u32, u32) {
    let (w, h) = img.dimensions();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found = false;

    for y in 0..h {
        for x in 0..w {
            if img.get_pixel(x, y)[3] > 20 {
                found = true;
                if x < min_x {
                    min_x = x;
                }
                if x > max_x {
                    max_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
    }

    if !found {
        (0, 0, w, h)
    } else {
        (min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)
    }
}

fn blit_scaled(
    dst: &mut RgbaImage,
    src: &RgbaImage,
    sx: u32,
    sy: u32,
    sw: u32,
    sh: u32,
    dx: i32,
    dy: i32,
    dw: u32,
    dh: u32,
) {
    if dw == 0 || dh == 0 || sw == 0 || sh == 0 {
        return;
    }
    let (dst_w, dst_h) = dst.dimensions();
    for py in 0..dh {
        let ty = dy + py as i32;
        if ty < 0 || ty >= dst_h as i32 {
            continue;
        }
        let src_y = sy + ((py as f32 / dh as f32) * sh as f32).min((sh - 1) as f32) as u32;
        for px in 0..dw {
            let tx = dx + px as i32;
            if tx < 0 || tx >= dst_w as i32 {
                continue;
            }
            let src_x = sx + ((px as f32 / dw as f32) * sw as f32).min((sw - 1) as f32) as u32;
            let pixel = *src.get_pixel(src_x, src_y);
            if pixel[3] > 0 {
                dst.put_pixel(tx as u32, ty as u32, pixel);
            }
        }
    }
}

fn slice_sheet_into_atlas(
    sheet: &RgbaImage,
    atlas: &mut RgbaImage,
    row_offset: u32,
    ref_standing_h: &mut f32,
) {
    let (sw, sh) = sheet.dimensions();
    let cell_w = sw / 5;
    let cell_h = sh / 2;

    for row in 0..2 {
        for col in 0..5 {
            let mut cell = image::imageops::crop_imm(
                sheet,
                col * cell_w,
                row * cell_h,
                cell_w,
                cell_h,
            )
            .to_image();
            remove_white_bg_floodfill(&mut cell, 232);
            let (bx, by, bw, bh) = opaque_bounds(&cell);

            if row_offset == 0 && row == 0 && col == 0 {
                *ref_standing_h = bh as f32;
            }

            let scale = ((SPRITE_H as f32 * 0.90) / (*ref_standing_h).max(1.0))
                .min((SPRITE_W as f32 * 0.92) / (bw as f32).max(1.0));
            let dw = ((bw as f32) * scale).round() as u32;
            let dh = ((bh as f32) * scale).round() as u32;

            let dst_cell_x = (col * SPRITE_W) as i32;
            let dst_cell_y = ((row_offset + row) * SPRITE_H) as i32;
            let dx = dst_cell_x + ((SPRITE_W as i32 - dw as i32) / 2);
            let dy = dst_cell_y + (SPRITE_H as i32 - dh as i32 - 6);

            blit_scaled(atlas, &cell, bx, by, bw, bh, dx, dy, dw, dh);
        }
    }
}

pub fn load_embedded_assets() -> GameAssets {
    let sheet_a = image::load_from_memory(include_bytes!("../../prototype/assets/unit_sheet_a.jpg"))
        .expect("unit_sheet_a.jpg")
        .to_rgba8();
    let sheet_b = image::load_from_memory(include_bytes!("../../prototype/assets/unit_sheet_b.jpg"))
        .expect("unit_sheet_b.jpg")
        .to_rgba8();

    let mut unit_atlas = RgbaImage::new(ATLAS_W, ATLAS_H);
    let mut ref_h = (sheet_a.height() / 2) as f32 * 0.82;
    slice_sheet_into_atlas(&sheet_a, &mut unit_atlas, 0, &mut ref_h);
    slice_sheet_into_atlas(&sheet_b, &mut unit_atlas, 2, &mut ref_h);

    let wall_raw = image::load_from_memory(include_bytes!("../../prototype/assets/wall.jpg"))
        .expect("wall.jpg")
        .to_rgba8();
    let wall_tex = image::imageops::resize(&wall_raw, 256, 256, image::imageops::FilterType::Triangle);

    let mut cover_raw = image::load_from_memory(include_bytes!("../../prototype/assets/cover.jpg"))
        .expect("cover.jpg")
        .to_rgba8();
    remove_white_bg_floodfill(&mut cover_raw, 232);
    let (cx, cy, cw, ch) = opaque_bounds(&cover_raw);
    let cover_crop = image::imageops::crop_imm(&cover_raw, cx, cy, cw, ch).to_image();
    let cover_tex = image::imageops::resize(&cover_crop, 256, 192, image::imageops::FilterType::Triangle);

    let mut weapon_raw = image::load_from_memory(include_bytes!("../../prototype/assets/weapon_fp.jpg"))
        .expect("weapon_fp.jpg")
        .to_rgba8();
    remove_white_bg_floodfill(&mut weapon_raw, 232);
    let weapon_tex = image::imageops::resize(&weapon_raw, 512, 340, image::imageops::FilterType::Triangle);

    GameAssets {
        unit_atlas,
        wall_tex,
        cover_tex,
        weapon_tex,
    }
}
