// Custom 2D/2.5D Engine Shaders (WGSL)
// 1) Textured/Colored 2D Quads (2.5D raycast strips, sky, ground, weapon, HUD, bitmap font)
// 2) 20-Frame Unit Shader with 2D Bone Skinning (LBS) + Breathing + Coat/Hair Sway + Hit Flash

struct QuadVertexIn {
    @location(0) pos: vec2<f32>,   // Pixel/screen coords or NDC-ready via viewport
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct QuadVertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct ScreenUniform {
    viewport: vec4<f32>, // (width, height, 0, 0)
};

@group(0) @binding(0)
var<uniform> screen: ScreenUniform;

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;

@vertex
fn vs_quad(in: QuadVertexIn) -> QuadVertexOut {
    var out: QuadVertexOut;
    let ndc_x = (in.pos.x / screen.viewport.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (in.pos.y / screen.viewport.y) * 2.0;
    out.clip_pos = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_quad(in: QuadVertexOut) -> @location(0) vec4<f32> {
    let tex = textureSample(t_diffuse, s_diffuse, in.uv);
    let col = tex * in.color;
    if (col.a < 0.02) {
        discard;
    }
    return col;
}

// --- Custom Unit Shader (20-Frame Atlas + 2D Bone Skinning + Breathing + Coat/Hair Sway) ---

struct UnitUniform {
    rect: vec4<f32>,       // (draw_x, draw_y, sprite_w, sprite_h)
    uv_rect: vec4<f32>,    // (u0, v0, u1, v1) in 5x4 atlas
    params: vec4<f32>,     // (time, breath_amp, sway_amp, hit_flash)
    clip_info: vec4<f32>,  // (flip_x, max_visible_ratio_y, 0, 0)
    // 4 bones * 2 rows each = 8 vec4s (row0 = [m00, m01, tx, 0], row1 = [m10, m11, ty, 0])
    bone_rows: array<vec4<f32>, 8>,
};

@group(2) @binding(0)
var<uniform> unit: UnitUniform;

struct UnitVertexIn {
    @location(0) local_pos: vec2<f32>,    // [0..1, 0..1]
    @location(1) bone_weights: vec4<f32>, // weights for bones 0(root), 1(torso), 2(head), 3(coat)
};

struct UnitVertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) local_y: f32,
};

fn transform_by_bone(bone_idx: u32, p: vec2<f32>) -> vec2<f32> {
    let r0 = unit.bone_rows[bone_idx * 2u];
    let r1 = unit.bone_rows[bone_idx * 2u + 1u];
    return vec2<f32>(
        r0.x * p.x + r0.y * p.y + r0.z,
        r1.x * p.x + r1.y * p.y + r1.z
    );
}

@vertex
fn vs_unit(in: UnitVertexIn) -> UnitVertexOut {
    var out: UnitVertexOut;
    let p = in.local_pos;

    // 1) 2D Linear Blend Skinning (4 bones)
    var skinned = vec2<f32>(0.0, 0.0);
    skinned += transform_by_bone(0u, p) * in.bone_weights.x;
    skinned += transform_by_bone(1u, p) * in.bone_weights.y;
    skinned += transform_by_bone(2u, p) * in.bone_weights.z;
    skinned += transform_by_bone(3u, p) * in.bone_weights.w;

    // 2) Custom Shader Breathing (chest expansion around y = 0.35..0.55)
    let t = unit.params.x;
    let breath_amp = unit.params.y;
    let chest_mask = exp(-pow((p.y - 0.42) * 4.5, 2.0));
    skinned.x += (p.x - 0.5) * sin(t * 2.5) * breath_amp * chest_mask;
    skinned.y -= sin(t * 2.5) * breath_amp * 0.35 * (1.0 - p.y);

    // 3) Custom Shader Coat / Hair Sway (feet at p.y=1.0 stay planted!)
    let sway_amp = unit.params.z;
    let foot_anchor = clamp(1.0 - p.y, 0.0, 1.0);
    let coat_mask = smoothstep(0.35, 0.72, p.y) * smoothstep(1.0, 0.85, p.y);
    let hair_mask = smoothstep(0.28, 0.05, p.y) * 0.55;
    let wave = sin(t * 4.2 + p.y * 7.0) * sway_amp * (coat_mask + hair_mask + foot_anchor * 0.18);
    skinned.x += wave;

    // Map [0..1] local_pos to screen pixel coordinates
    let px = unit.rect.x + skinned.x * unit.rect.z;
    let py = unit.rect.y + skinned.y * unit.rect.w;

    let ndc_x = (px / screen.viewport.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (py / screen.viewport.y) * 2.0;
    out.clip_pos = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);

    let u_raw = select(p.x, 1.0 - p.x, unit.clip_info.x > 0.5);
    out.uv = vec2<f32>(
        mix(unit.uv_rect.x, unit.uv_rect.z, u_raw),
        mix(unit.uv_rect.y, unit.uv_rect.w, p.y)
    );
    out.local_y = p.y;
    return out;
}

@fragment
fn fs_unit(in: UnitVertexOut) -> @location(0) vec4<f32> {
    if (in.local_y > unit.clip_info.y) {
        discard;
    }
    var col = textureSample(t_diffuse, s_diffuse, in.uv);
    if (col.a < 0.08) {
        discard;
    }
    let flash = unit.params.w;
    if (flash > 0.01) {
        col = vec4<f32>(mix(col.rgb, vec3<f32>(1.0, 0.4, 0.25), flash), col.a);
    }
    return col;
}
