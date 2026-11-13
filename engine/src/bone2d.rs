// Simple, patent-safe 2D Bone Hierarchy & Linear Blend Skinning (LBS)

#[derive(Clone, Copy, Debug)]
pub struct Mat2D {
    // Column-major 2x3 affine: [m00, m10, m01, m11, tx, ty]
    pub m: [f32; 6],
}

impl Mat2D {
    pub const IDENTITY: Self = Self {
        m: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    };

    pub fn from_trs(tx: f32, ty: f32, rot: f32, sx: f32, sy: f32) -> Self {
        let (s, c) = rot.sin_cos();
        Self {
            m: [c * sx, s * sx, -s * sy, c * sy, tx, ty],
        }
    }

    pub fn mul(self, rhs: Self) -> Self {
        let a = self.m;
        let b = rhs.m;
        Self {
            m: [
                a[0] * b[0] + a[2] * b[1],
                a[1] * b[0] + a[3] * b[1],
                a[0] * b[2] + a[2] * b[3],
                a[1] * b[2] + a[3] * b[3],
                a[0] * b[4] + a[2] * b[5] + a[4],
                a[1] * b[4] + a[3] * b[5] + a[5],
            ],
        }
    }

    pub fn inverse(self) -> Self {
        let [a, b, c, d, tx, ty] = self.m;
        let det = a * d - b * c;
        if det.abs() < 1e-8 {
            return Self::IDENTITY;
        }
        let inv = 1.0 / det;
        let na = d * inv;
        let nb = -b * inv;
        let nc = -c * inv;
        let nd = a * inv;
        Self {
            m: [
                na,
                nb,
                nc,
                nd,
                -(na * tx + nc * ty),
                -(nb * tx + nd * ty),
            ],
        }
    }

    pub fn transform_point(self, x: f32, y: f32) -> [f32; 2] {
        let [a, b, c, d, tx, ty] = self.m;
        [a * x + c * y + tx, b * x + d * y + ty]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Bone2D {
    pub parent: Option<usize>,
    pub bind_pos: [f32; 2],
    pub pos: [f32; 2],
    pub rot: f32,
    pub scale: [f32; 2],
}

impl Bone2D {
    pub fn new(parent: Option<usize>, bind_pos: [f32; 2]) -> Self {
        Self {
            parent,
            bind_pos,
            pos: bind_pos,
            rot: 0.0,
            scale: [1.0, 1.0],
        }
    }
}

pub const MAX_BONES: usize = 4;

#[derive(Clone, Debug)]
pub struct Skeleton2D {
    pub bones: Vec<Bone2D>,
    pub inv_bind: Vec<Mat2D>,
    pub skin_mats: [Mat2D; MAX_BONES],
}

impl Skeleton2D {
    pub fn new(bones: Vec<Bone2D>) -> Self {
        let mut world = vec![Mat2D::IDENTITY; bones.len()];
        for (i, b) in bones.iter().enumerate() {
            let local = Mat2D::from_trs(b.bind_pos[0], b.bind_pos[1], 0.0, 1.0, 1.0);
            world[i] = match b.parent {
                Some(p) => world[p].mul(local),
                None => local,
            };
        }
        let inv_bind = world.iter().map(|m| m.inverse()).collect();
        Self {
            bones,
            inv_bind,
            skin_mats: [Mat2D::IDENTITY; MAX_BONES],
        }
    }

    pub fn update(&mut self) {
        let mut world = [Mat2D::IDENTITY; MAX_BONES];
        for (i, b) in self.bones.iter().enumerate().take(MAX_BONES) {
            let local = Mat2D::from_trs(b.pos[0], b.pos[1], b.rot, b.scale[0], b.scale[1]);
            world[i] = match b.parent {
                Some(p) if p < i => world[p].mul(local),
                _ => local,
            };
            self.skin_mats[i] = world[i].mul(self.inv_bind[i]);
        }
    }

    /// Pack skin matrices into GPU-friendly [vec4; 2] per bone:
    /// row0 = [m00, m01, tx, 0], row1 = [m10, m11, ty, 0]
    pub fn gpu_rows(&self) -> [[f32; 4]; MAX_BONES * 2] {
        let mut out = [[0.0_f32; 4]; MAX_BONES * 2];
        for i in 0..MAX_BONES {
            let m = self.skin_mats[i].m;
            out[i * 2] = [m[0], m[2], m[4], 0.0];
            out[i * 2 + 1] = [m[1], m[3], m[5], 0.0];
        }
        out
    }
}

/// Standard 2-bone unit skeleton:
/// Bone 0: Root (feet at y=1.0 in UV space)
/// Bone 1: Torso/Upper body (y=0.55)
/// Bone 2: Head (y=0.22)
/// Bone 3: Coat/Sway hem (y=0.68)
pub fn make_unit_skeleton() -> Skeleton2D {
    Skeleton2D::new(vec![
        Bone2D::new(None, [0.5, 1.0]),
        Bone2D::new(Some(0), [0.0, -0.45]),
        Bone2D::new(Some(1), [0.0, -0.33]),
        Bone2D::new(Some(1), [0.0, 0.13]),
    ])
}

/// 2-bone first-person weapon skeleton:
/// Bone 0: Arm/Shoulder
/// Bone 1: Rifle Receiver/Barrel
pub fn make_weapon_skeleton() -> Skeleton2D {
    Skeleton2D::new(vec![
        Bone2D::new(None, [0.8, 0.85]),
        Bone2D::new(Some(0), [-0.35, -0.25]),
    ])
}
