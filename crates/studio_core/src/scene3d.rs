//! A 3D scene seen from outside: which model lies under a point of its
//! picture, and where the axes of a model go on that picture. Pure, so a
//! page's session and an agent's get the answers the window's canvas draws
//! from.
//!
//! The running preview says what it knows (a camera, the models, each one's
//! place in the scene); the ray is cast here, because a preview's own `pick`
//! returns an object no caller can read.

use schemars::JsonSchema;
use serde::Serialize;

use crate::{place::Rect, preview::Declared};

pub type Vec3 = [f64; 3];

/// A rotation as the preview states it: w, x, y, z.
pub type Quat = [f64; 4];

/// What a camera tells of where the view stands.
#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
    pub position: Vec3,
    pub rotation: Quat,
    /// Degrees, over the view's height: the orientation a camera starts with.
    pub field_of_view: f64,
    /// An orthographic camera has no point the rays leave from.
    pub perspective: bool,
}

/// A model of the scene, as placed in it.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub declared: Declared,
    /// The mesh it shows: `#Cube`, a file…
    pub source: String,
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    /// Where the world goes for one unit of its own `x`, `y` and `z`: its
    /// parent's rotation and scale. The identity for a model that sits
    /// straight in the view.
    pub axes: [Vec3; 3],
}

/// A `View3D` with the camera it looks through and its models.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// In the window, in logical pixels.
    pub view: Rect,
    pub camera: Option<Camera>,
    pub bodies: Vec<Body>,
}

/// The half sides of what a built-in mesh fills, before the model's scale. A
/// mesh of a file is not here: its extent is not known from outside, so it is
/// never picked by a click.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Shape {
    Box(Vec3),
    Sphere(f64),
}

fn shape_of(source: &str) -> Option<Shape> {
    // The primitives are 100 units across, as Qt Quick 3D's are.
    match source {
        "#Cube" => Some(Shape::Box([50.0; 3])),
        "#Sphere" => Some(Shape::Sphere(50.0)),
        // Their bounding boxes: a click at a corner the shape does not fill
        // takes the model, which no one aims at by mistake.
        "#Cylinder" | "#Cone" => Some(Shape::Box([50.0; 3])),
        "#Rectangle" => Some(Shape::Box([50.0, 50.0, 0.5])),
        _ => None,
    }
}

fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn norm(a: Vec3) -> Vec3 {
    let n = dot(a, a).sqrt();
    if n == 0.0 {
        a
    } else {
        [a[0] / n, a[1] / n, a[2] / n]
    }
}

/// `v` turned by `q`.
pub fn rotate(q: Quat, v: Vec3) -> Vec3 {
    let [w, x, y, z] = q;
    let n = (w * w + x * x + y * y + z * z).sqrt();
    let (w, x, y, z) = if n == 0.0 {
        (1.0, 0.0, 0.0, 0.0)
    } else {
        (w / n, x / n, y / n, z / n)
    };
    let u = [x, y, z];
    let t = [
        2.0 * (u[1] * v[2] - u[2] * v[1]),
        2.0 * (u[2] * v[0] - u[0] * v[2]),
        2.0 * (u[0] * v[1] - u[1] * v[0]),
    ];
    [
        v[0] + w * t[0] + (u[1] * t[2] - u[2] * t[1]),
        v[1] + w * t[1] + (u[2] * t[0] - u[0] * t[2]),
        v[2] + w * t[2] + (u[0] * t[1] - u[1] * t[0]),
    ]
}

fn conjugate(q: Quat) -> Quat {
    [q[0], -q[1], -q[2], -q[3]]
}

/// The camera's own axes in the scene.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Eye {
    // The checker of answers knows no fixed-length arrays: a list of three.
    #[schemars(with = "Vec<f64>")]
    pub position: Vec3,
    #[schemars(with = "Vec<f64>")]
    pub right: Vec3,
    #[schemars(with = "Vec<f64>")]
    pub up: Vec3,
    #[schemars(with = "Vec<f64>")]
    pub forward: Vec3,
    /// Half the view's height at one unit ahead.
    pub tan_y: f64,
    /// The view's width over its height.
    pub aspect: f64,
}

impl Camera {
    /// The eye as seen through a view of `width` by `height`; `None` for a
    /// camera the rays cannot be cast from.
    pub fn eye(&self, width: f64, height: f64) -> Option<Eye> {
        if !self.perspective || width <= 0.0 || height <= 0.0 {
            return None;
        }
        let aspect = width / height;
        let tan_y = (self.field_of_view.to_radians() / 2.0).tan();
        Some(Eye {
            position: self.position,
            right: rotate(self.rotation, [1.0, 0.0, 0.0]),
            up: rotate(self.rotation, [0.0, 1.0, 0.0]),
            forward: rotate(self.rotation, [0.0, 0.0, -1.0]),
            tan_y,
            aspect,
        })
    }
}

impl Eye {
    /// The ray through the pixel `x`, `y` of a view `width` by `height`.
    pub fn ray(&self, x: f64, y: f64, width: f64, height: f64) -> (Vec3, Vec3) {
        let nx = 2.0 * x / width - 1.0;
        let ny = 1.0 - 2.0 * y / height;
        let (sx, sy) = (nx * self.tan_y * self.aspect, ny * self.tan_y);
        let d = [
            self.forward[0] + self.right[0] * sx + self.up[0] * sy,
            self.forward[1] + self.right[1] * sx + self.up[1] * sy,
            self.forward[2] + self.right[2] * sx + self.up[2] * sy,
        ];
        (self.position, norm(d))
    }
}

impl Body {
    /// How far along the ray this model is first hit, if it is.
    fn hit(&self, origin: Vec3, dir: Vec3) -> Option<f64> {
        let shape = shape_of(&self.source)?;
        let inverse = conjugate(self.rotation);
        let local = |v: Vec3| {
            let r = rotate(inverse, v);
            [
                r[0] / self.scale[0],
                r[1] / self.scale[1],
                r[2] / self.scale[2],
            ]
        };
        // An affine map keeps the ray's parameter, so the distance found in
        // the model's own space is the one along the ray.
        let o = local(sub(origin, self.position));
        let d = local(dir);
        match shape {
            Shape::Box(half) => slab(o, d, half),
            Shape::Sphere(radius) => {
                let a = dot(d, d);
                let b = dot(o, d);
                let c = dot(o, o) - radius * radius;
                let disc = b * b - a * c;
                if a == 0.0 || disc < 0.0 {
                    return None;
                }
                let root = disc.sqrt();
                [(-b - root) / a, (-b + root) / a]
                    .into_iter()
                    .find(|t| *t >= 0.0)
            }
        }
    }
}

fn slab(o: Vec3, d: Vec3, half: Vec3) -> Option<f64> {
    let (mut near, mut far) = (f64::NEG_INFINITY, f64::INFINITY);
    for i in 0..3 {
        if d[i].abs() < 1e-12 {
            if o[i].abs() > half[i] {
                return None;
            }
            continue;
        }
        let (a, b) = ((-half[i] - o[i]) / d[i], (half[i] - o[i]) / d[i]);
        near = near.max(a.min(b));
        far = far.min(a.max(b));
    }
    (near <= far && far >= 0.0).then_some(near.max(0.0))
}

/// Why nothing under a point can be named: the reason is for the person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blind {
    /// The view has no camera of its own, or an orthographic one.
    NoCamera,
}

impl Scene {
    /// The model the pixel `x`, `y` of the window lies on, nearest first;
    /// `Ok(None)` when it lies on none of them.
    ///
    /// # Errors
    /// [`Blind`] when the scene has no camera a ray can be cast from.
    pub fn pick(&self, x: f64, y: f64) -> Result<Option<&Body>, Blind> {
        let eye = self
            .camera
            .as_ref()
            .and_then(|c| c.eye(self.view.width, self.view.height))
            .ok_or(Blind::NoCamera)?;
        let (origin, dir) = eye.ray(
            x - self.view.x,
            y - self.view.y,
            self.view.width,
            self.view.height,
        );
        Ok(self
            .bodies
            .iter()
            .filter_map(|b| b.hit(origin, dir).map(|t| (t, b)))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, b)| b))
    }

    /// The body declared at `at`.
    pub fn body(&self, at: &Declared) -> Option<&Body> {
        self.bodies.iter().find(|b| &b.declared == at)
    }

    /// What the canvas needs to draw and drag the axes of `body`.
    pub fn handles(&self, body: &Body) -> Option<Handles> {
        let eye = self
            .camera
            .as_ref()?
            .eye(self.view.width, self.view.height)?;
        Some(Handles {
            view: self.view,
            eye,
            origin: body.position,
            axes: body.axes,
        })
    }
}

/// The axes of a model on the view, for the canvas: it projects and casts
/// rays itself, so a drag follows the pointer without asking the preview.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Handles {
    /// The view, in the window.
    pub view: Rect,
    pub eye: Eye,
    /// The model's place in the scene.
    // The checker of answers knows no fixed-length arrays: a list of three.
    #[schemars(with = "Vec<f64>")]
    pub origin: Vec3,
    /// The scene's movement for one unit of the model's `x`, `y` and `z`.
    #[schemars(with = "Vec<Vec<f64>>")]
    pub axes: [Vec3; 3],
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declared(line: u32) -> Declared {
        Declared {
            file: "Main.qml".to_owned(),
            line,
            column: 1,
        }
    }

    fn cube(line: u32, position: Vec3) -> Body {
        Body {
            declared: declared(line),
            source: "#Cube".to_owned(),
            position,
            rotation: [1.0, 0.0, 0.0, 0.0],
            scale: [1.0; 3],
            axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    fn scene(bodies: Vec<Body>) -> Scene {
        Scene {
            view: Rect {
                x: 0.0,
                y: 0.0,
                width: 320.0,
                height: 240.0,
            },
            camera: Some(Camera {
                position: [0.0, 0.0, 300.0],
                rotation: [1.0, 0.0, 0.0, 0.0],
                field_of_view: 60.0,
                perspective: true,
            }),
            bodies,
        }
    }

    #[test]
    fn the_middle_of_the_view_is_on_the_cube_in_front_of_the_camera() {
        let s = scene(vec![cube(1, [0.0; 3])]);
        assert_eq!(s.pick(160.0, 120.0).unwrap().unwrap().declared.line, 1);
    }

    #[test]
    fn the_corner_of_the_view_is_on_nothing() {
        let s = scene(vec![cube(1, [0.0; 3])]);
        assert!(s.pick(4.0, 4.0).unwrap().is_none());
    }

    #[test]
    fn the_front_face_ends_where_the_field_of_view_puts_it() {
        // 250 units off, 60 degrees over 240 pixels: a 100-unit face is
        // 83 pixels across, so 41 from the middle.
        let s = scene(vec![cube(1, [0.0; 3])]);
        assert!(s.pick(160.0 + 40.0, 120.0).unwrap().is_some());
        assert!(s.pick(160.0 + 44.0, 120.0).unwrap().is_none());
    }

    #[test]
    fn the_nearer_of_two_models_is_the_one_picked() {
        let s = scene(vec![cube(1, [0.0, 0.0, -200.0]), cube(2, [0.0, 0.0, 0.0])]);
        assert_eq!(s.pick(160.0, 120.0).unwrap().unwrap().declared.line, 2);
    }

    #[test]
    fn a_model_behind_the_camera_is_not_picked() {
        let s = scene(vec![cube(1, [0.0, 0.0, 600.0])]);
        assert!(s.pick(160.0, 120.0).unwrap().is_none());
    }

    #[test]
    fn a_scaled_and_turned_cube_is_hit_where_it_is() {
        let mut b = cube(1, [0.0; 3]);
        b.scale = [3.0, 1.0, 1.0];
        // A quarter turn about y puts the long side along z.
        let h = std::f64::consts::FRAC_1_SQRT_2;
        b.rotation = [h, 0.0, h, 0.0];
        let s = scene(vec![b]);
        assert!(s.pick(160.0 + 40.0, 120.0).unwrap().is_some());
    }

    #[test]
    fn a_sphere_does_not_fill_its_bounding_box() {
        let mut b = cube(1, [0.0; 3]);
        b.source = "#Sphere".to_owned();
        let s = scene(vec![b]);
        assert!(s.pick(160.0, 120.0).unwrap().is_some());
        assert!(s.pick(160.0 + 38.0, 120.0 + 28.0).unwrap().is_none());
    }

    #[test]
    fn a_mesh_of_a_file_is_not_picked() {
        let mut b = cube(1, [0.0; 3]);
        b.source = "meshes/ship.mesh".to_owned();
        let s = scene(vec![b]);
        assert!(s.pick(160.0, 120.0).unwrap().is_none());
    }

    #[test]
    fn an_orthographic_camera_cannot_be_cast_from() {
        let mut s = scene(vec![cube(1, [0.0; 3])]);
        s.camera.as_mut().unwrap().perspective = false;
        assert_eq!(s.pick(160.0, 120.0), Err(Blind::NoCamera));
    }

    #[test]
    fn a_view_beside_the_window_s_corner_is_picked_in_window_pixels() {
        let mut s = scene(vec![cube(1, [0.0; 3])]);
        s.view.x = 100.0;
        s.view.y = 50.0;
        assert!(s.pick(260.0, 170.0).unwrap().is_some());
    }
}
