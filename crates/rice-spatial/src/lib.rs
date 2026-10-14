//! Allocation-bounded scene math for the RiceWM spatial presenter.
//! Coordinates: metres, Y up, forward -Z; matrices are column-major OpenGL.
//! No window pixels, desktop permissions, GPU bindings, or world artwork live here.

use std::cell::RefCell;

pub const MAX_COLLIDERS: usize = 512;
pub const MAX_PLANES: usize = 128;
const STEP: f32 = 1.0 / 120.0;
const RADIUS: f32 = 0.28;
const EYE: f32 = 1.7;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3(pub f32, pub f32, pub f32);
impl Vec3 {
    pub fn dot(self, b: Self) -> f32 {
        self.0 * b.0 + self.1 * b.1 + self.2 * b.2
    }
    pub fn cross(self, b: Self) -> Self {
        Self(
            self.1 * b.2 - self.2 * b.1,
            self.2 * b.0 - self.0 * b.2,
            self.0 * b.1 - self.1 * b.0,
        )
    }
    pub fn plus(self, b: Self) -> Self {
        Self(self.0 + b.0, self.1 + b.1, self.2 + b.2)
    }
    pub fn minus(self, b: Self) -> Self {
        self.plus(b.scale(-1.0))
    }
    pub fn scale(self, n: f32) -> Self {
        Self(self.0 * n, self.1 * n, self.2 * n)
    }
    fn valid(self) -> bool {
        [self.0, self.1, self.2]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 100_000.0)
    }
    fn unit(self) -> Option<Self> {
        let length = self.dot(self).sqrt();
        (self.valid() && length > 1e-6).then(|| self.scale(1.0 / length))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Collider {
    pub min: Vec3,
    pub max: Vec3,
}
#[derive(Clone, Copy, Debug)]
pub struct OrientedCollider { center: Vec3, half: Vec3, sin: f32, cos: f32 }
impl OrientedCollider {
    fn local(self,p:Vec3)->Vec3 {let d=p.minus(self.center);Vec3(self.cos*d.0-self.sin*d.2,p.1,self.sin*d.0+self.cos*d.2)}
    fn local_delta(self,d:Vec3)->Vec3 {Vec3(self.cos*d.0-self.sin*d.2,d.1,self.sin*d.0+self.cos*d.2)}
    fn normal(self,d:Vec3)->Vec3 {Vec3(self.cos*d.0+self.sin*d.2,d.1,-self.sin*d.0+self.cos*d.2)}
    fn bounds(self)->Collider {Collider{min:Vec3(-self.half.0,self.center.1-self.half.1,-self.half.2),max:Vec3(self.half.0,self.center.1+self.half.1,self.half.2)}}
}
#[derive(Clone, Copy, Debug)]
pub struct Plane {
    pub id: u32,
    pub center: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub width: f32,
    pub height: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub id: u32,
    pub distance: f32,
    pub u: f32,
    pub v: f32,
}

pub struct Scene {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    colliders: Vec<Collider>,
    oriented: Vec<OrientedCollider>,
    planes: Vec<Plane>,
    remainder: f32,
    velocity_y: f32,
    jump_held: bool,
    flying: bool,
    grounded: bool,
    matrix: [f32; 16],
    camera: [f32; 8],
    hit: [f32; 4],
}
impl Default for Scene {
    fn default() -> Self {
        Self {
            position: Vec3(0.0, EYE, 6.0),
            yaw: 0.0,
            pitch: 0.0,
            colliders: Vec::with_capacity(MAX_COLLIDERS),
            oriented: Vec::with_capacity(MAX_COLLIDERS),
            planes: Vec::with_capacity(MAX_PLANES),
            remainder: 0.0,
            velocity_y: 0.0,
            jump_held: false,
            flying: false,
            grounded: true,
            matrix: [0.0; 16],
            camera: [0.0; 8],
            hit: [0.0; 4],
        }
    }
}
impl Scene {
    pub fn set_pose(&mut self, position: Vec3, yaw: f32, pitch: f32) -> bool {
        if !position.valid() || !yaw.is_finite() || !pitch.is_finite() {
            return false;
        }
        self.position = Vec3(position.0, position.1.max(EYE), position.2);
        self.yaw =
            (yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        self.pitch = pitch.clamp(-1.48, 1.48);
        self.remainder = 0.0;
        self.velocity_y = 0.0;
        self.jump_held = false;
        self.grounded = self.position.1 <= EYE;
        true
    }
    pub fn look(&mut self, dx: f32, dy: f32) {
        if !dx.is_finite() || !dy.is_finite() {
            return;
        }
        self.yaw = (self.yaw + dx.clamp(-2000.0, 2000.0) * 0.002 + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        self.pitch = (self.pitch - dy.clamp(-2000.0, 2000.0) * 0.002).clamp(-1.48, 1.48);
    }
    pub fn forward(&self) -> Vec3 {
        Vec3(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }
    pub fn add_collider(&mut self, b: Collider) -> bool {
        if self.colliders.len()+self.oriented.len() >= MAX_COLLIDERS
            || !b.min.valid()
            || !b.max.valid()
            || b.max.0 <= b.min.0
            || b.max.1 <= b.min.1
            || b.max.2 <= b.min.2
        {
            return false;
        }
        self.colliders.push(b);
        true
    }
    pub fn add_oriented_collider(&mut self,center:Vec3,size:Vec3,yaw:f32)->bool {
        if self.colliders.len()+self.oriented.len()>=MAX_COLLIDERS || !center.valid() || !size.valid() || size.0<=0.0 || size.1<=0.0 || size.2<=0.0 || !yaw.is_finite() || yaw.abs()>std::f32::consts::PI {return false;}
        self.oriented.push(OrientedCollider{center,half:size.scale(0.5),sin:yaw.sin(),cos:yaw.cos()});true
    }
    pub fn clear_planes(&mut self) {
        self.planes.clear();
    }
    pub fn add_plane(&mut self, p: Plane) -> bool {
        if self.planes.len() >= MAX_PLANES
            || p.id == 0
            || p.id > 16_777_215
            || !p.center.valid()
            || !p.width.is_finite()
            || !p.height.is_finite()
            || p.width <= 0.0
            || p.height <= 0.0
            || p.width > 1000.0
            || p.height > 1000.0
            || !p.right.valid()
            || !p.up.valid()
            || (p.right.dot(p.right) - 1.0).abs() > 0.001
            || (p.up.dot(p.up) - 1.0).abs() > 0.001
            || p.right.dot(p.up).abs() > 0.001
            || self.planes.iter().any(|x| x.id == p.id)
        {
            return false;
        }
        self.planes.push(p);
        true
    }
    pub fn pick(&self, origin: Vec3, direction: Vec3) -> Option<Hit> {
        if !origin.valid() {
            return None;
        }
        let dir = direction.unit()?;
        let mut best: Option<Hit> = None;
        for p in &self.planes {
            let n = p.right.cross(p.up);
            let denom = dir.dot(n);
            // Only the front is interactive, matching the visible app face.
            if denom >= -1e-5 {
                continue;
            }
            let t = p.center.minus(origin).dot(n) / denom;
            if t <= 0.0 || t > 500.0 || best.is_some_and(|b| t >= b.distance) {
                continue;
            }
            let local = origin.plus(dir.scale(t)).minus(p.center);
            let u = local.dot(p.right) / p.width + 0.5;
            let v = 0.5 - local.dot(p.up) / p.height;
            if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
                best = Some(Hit {
                    id: p.id,
                    distance: t,
                    u,
                    v,
                });
            }
        }
        best
    }
    pub fn set_flight(&mut self, enabled: bool) {
        if self.flying == enabled { return; }
        self.flying = enabled;
        self.velocity_y = 0.0;
        self.jump_held = false;
    }
    pub fn clear_colliders(&mut self) { self.colliders.clear(); self.oriented.clear(); }
    pub fn screen_pick(&self, x: f32, y: f32, aspect: f32, fov: f32) -> Option<Hit> {
        if ![x,y,aspect,fov].iter().all(|v| v.is_finite()) || aspect <= 0.0 { return None; }
        let right = Vec3(self.yaw.cos(), 0.0, self.yaw.sin());
        let up = right.cross(self.forward());
        let scale = (fov.clamp(0.4,2.4)*0.5).tan();
        self.pick(self.position, self.forward().plus(right.scale(x*aspect*scale)).plus(up.scale(y*scale)))
    }
    pub fn step(&mut self, dt: f32, forward: f32, right: f32, sprint: bool, jump: bool) {
        self.step_vertical(dt,forward,right,sprint,jump,0.0);
    }
    pub fn step_vertical(&mut self, dt: f32, forward: f32, right: f32, sprint: bool, jump: bool, vertical: f32) {
        if !dt.is_finite() || dt <= 0.0 || !forward.is_finite() || !right.is_finite() || !vertical.is_finite() { return; }
        let f=forward.clamp(-1.0,1.0);let r=right.clamp(-1.0,1.0);let vertical=vertical.clamp(-1.0,1.0);
        let direction = if self.flying { self.forward() } else { Vec3(self.yaw.sin(),0.0,-self.yaw.cos()) };
        let delta = direction.scale(f).plus(Vec3(self.yaw.cos()*r,if self.flying {vertical} else {0.0},self.yaw.sin()*r));
        let delta=delta.scale((if sprint {11.0} else {5.2})*STEP/delta.dot(delta).sqrt().max(1.0));
        if !self.flying && jump && !self.jump_held && self.grounded { self.velocity_y=5.0;self.grounded=false; }
        self.jump_held=jump;
        self.remainder=(self.remainder+dt.min(0.1)).min(0.1);
        let count=((self.remainder+1e-7)/STEP).floor().min(12.0) as u32;
        for _ in 0..count {
            self.slide(Vec3(delta.0,0.0,delta.2));
            let dy=if self.flying { delta.1 } else {self.velocity_y-=12.0*STEP;self.velocity_y*STEP};
            if self.move_vertical(dy) {self.velocity_y=0.0;}
        }
        self.remainder=(self.remainder-count as f32*STEP).max(0.0);
    }
    fn move_vertical(&mut self, dy: f32) -> bool {
        let old=self.position.1;let requested=old+dy;let mut next=requested.clamp(EYE,5000.0);
        // Float rounding grows with altitude. Only an actual boundary or swept
        // collider contact may stop gravity; subtracting positions falsely
        // classified each high-altitude substep as a landing.
        let mut stopped=requested<EYE || requested>5000.0;
        for b in &self.colliders {
            if self.position.0 <= b.min.0-RADIUS || self.position.0 >= b.max.0+RADIUS ||
                self.position.2 <= b.min.2-RADIUS || self.position.2 >= b.max.2+RADIUS {continue;}
            if dy<0.0 && old>=b.max.1+EYE-0.001 && next<b.max.1+EYE {next=next.max(b.max.1+EYE);stopped=true;}
            if dy>0.0 && old<=b.min.1-0.12+0.001 && next>b.min.1-0.12 {next=next.min(b.min.1-0.12);stopped=true;}
        }
        for b in &self.oriented {
            let p=b.local(self.position);let bounds=b.bounds();
            if p.0<=bounds.min.0-RADIUS || p.0>=bounds.max.0+RADIUS || p.2<=bounds.min.2-RADIUS || p.2>=bounds.max.2+RADIUS {continue;}
            if dy<0.0 && old>=bounds.max.1+EYE-0.001 && next<bounds.max.1+EYE {next=next.max(bounds.max.1+EYE);stopped=true;}
            if dy>0.0 && old<=bounds.min.1-0.12+0.001 && next>bounds.min.1-0.12 {next=next.min(bounds.min.1-0.12);stopped=true;}
        }
        self.position.1=next;
        self.grounded=dy<=0.0 && stopped;
        stopped
    }
    fn slide(&mut self, mut delta: Vec3) {
        // Sweep a horizontal character box against expanded static boxes.
        // A continuous sweep prevents tunnelling through thin walls at sprint.
        for _ in 0..3 {
            let mut time = 1.0;
            let mut normal = Vec3::default();
            for b in &self.colliders {
                if self.position.1 <= b.min.1 || self.position.1 - EYE >= b.max.1 {
                    continue;
                }
                if let Some((t, n)) = sweep(self.position, delta, *b) {
                    if t < time {
                        time = t;
                        normal = n;
                    }
                }
            }
            for b in &self.oriented {
                let bounds=b.bounds();if self.position.1<=bounds.min.1 || self.position.1-EYE>=bounds.max.1 {continue;}
                if let Some((t,n))=sweep(b.local(self.position),b.local_delta(delta),bounds) {if t<time {time=t;normal=b.normal(n);}}
            }
            self.position = self.position.plus(delta.scale((time - 0.0001).max(0.0)));
            if time >= 1.0 {
                break;
            }
            delta = delta.scale(1.0 - time);
            delta = delta.minus(normal.scale(delta.dot(normal)));
            if delta.dot(delta) < 1e-10 {
                break;
            }
        }
    }
    pub fn view_projection(&mut self, aspect: f32, fov: f32) -> &[f32; 16] {
        let aspect = if aspect.is_finite() {
            aspect.clamp(0.1, 10.0)
        } else {
            1.0
        };
        let fov = if fov.is_finite() {
            fov.clamp(0.4, 2.4)
        } else {
            1.3
        };
        let forward = self.forward();
        let right = Vec3(self.yaw.cos(), 0.0, self.yaw.sin());
        let up = right.cross(forward);
        let back = forward.scale(-1.0);
        let v = [
            right.0,
            up.0,
            back.0,
            0.0,
            right.1,
            up.1,
            back.1,
            0.0,
            right.2,
            up.2,
            back.2,
            0.0,
            -right.dot(self.position),
            -up.dot(self.position),
            -back.dot(self.position),
            1.0,
        ];
        let f = 1.0 / (fov * 0.5).tan();
        let near = 0.08;
        let far = 12000.0;
        let p = [
            f / aspect,
            0.0,
            0.0,
            0.0,
            0.0,
            f,
            0.0,
            0.0,
            0.0,
            0.0,
            (far + near) / (near - far),
            -1.0,
            0.0,
            0.0,
            2.0 * far * near / (near - far),
            0.0,
        ];
        for c in 0..4 {
            for r in 0..4 {
                self.matrix[c * 4 + r] = (0..4).map(|k| p[k * 4 + r] * v[c * 4 + k]).sum();
            }
        }
        &self.matrix
    }
}
fn sweep(pos: Vec3, d: Vec3, b: Collider) -> Option<(f32, Vec3)> {
    let mut enter: f32 = 0.0;
    let mut exit: f32 = 1.0;
    let mut normal = Vec3::default();
    for (origin, delta, min, max, axis) in [
        (
            pos.0,
            d.0,
            b.min.0 - RADIUS,
            b.max.0 + RADIUS,
            Vec3(1.0, 0.0, 0.0),
        ),
        (
            pos.2,
            d.2,
            b.min.2 - RADIUS,
            b.max.2 + RADIUS,
            Vec3(0.0, 0.0, 1.0),
        ),
    ] {
        if delta.abs() < 1e-8 {
            if origin <= min || origin >= max {
                return None;
            }
            continue;
        }
        let a = (min - origin) / delta;
        let z = (max - origin) / delta;
        let lo = a.min(z);
        let hi = a.max(z);
        if lo >= enter {
            enter = lo;
            normal = axis.scale(-delta.signum());
        }
        exit = exit.min(hi);
        if enter > exit {
            return None;
        }
    }
    (enter >= 0.0 && enter < 1.0 && exit > 0.0 && normal.dot(normal) > 0.0)
        .then_some((enter, normal))
}

// One private scene per Wasm instance. Output pointers reference stable fixed
// buffers and are valid until the next call; JS must refresh views after grow.
thread_local! { static SCENE: RefCell<Scene> = RefCell::new(Scene::default()); }
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_version() -> u32 {
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_reset() {
    SCENE.with(|s| *s.borrow_mut() = Scene::default());
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_pose(x: f32, y: f32, z: f32, yaw: f32, pitch: f32) -> u32 {
    SCENE.with(|s| u32::from(s.borrow_mut().set_pose(Vec3(x, y, z), yaw, pitch)))
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_look(dx: f32, dy: f32) {
    SCENE.with(|s| s.borrow_mut().look(dx, dy));
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_step(dt: f32, f: f32, r: f32, sprint: u32, jump: u32) {
    SCENE.with(|s| s.borrow_mut().step(dt, f, r, sprint != 0, jump != 0));
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_flight(enabled: u32) { SCENE.with(|s| s.borrow_mut().set_flight(enabled!=0)); }
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_step_vertical(dt:f32,f:f32,r:f32,sprint:u32,jump:u32,vertical:f32) {
    SCENE.with(|s| s.borrow_mut().step_vertical(dt,f,r,sprint!=0,jump!=0,vertical));
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_clear_boxes() { SCENE.with(|s| s.borrow_mut().clear_colliders()); }
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_pick_at(x:f32,y:f32,aspect:f32,fov:f32) -> *const f32 {
    SCENE.with(|s| {let mut s=s.borrow_mut();s.hit=s.screen_pick(x,y,aspect,fov).map_or([0.0;4],|h| [h.id as f32,h.distance,h.u,h.v]);s.hit.as_ptr()})
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_box(x: f32, y: f32, z: f32, a: f32, b: f32, c: f32) -> u32 {
    SCENE.with(|s| {
        u32::from(s.borrow_mut().add_collider(Collider {
            min: Vec3(x, y, z),
            max: Vec3(a, b, c),
        }))
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_obb(x:f32,y:f32,z:f32,w:f32,h:f32,d:f32,yaw:f32)->u32 {
    SCENE.with(|s|u32::from(s.borrow_mut().add_oriented_collider(Vec3(x,y,z),Vec3(w,h,d),yaw)))
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_clear_planes() {
    SCENE.with(|s| s.borrow_mut().clear_planes());
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_plane(
    id: u32,
    x: f32,
    y: f32,
    z: f32,
    rx: f32,
    ry: f32,
    rz: f32,
    ux: f32,
    uy: f32,
    uz: f32,
    w: f32,
    h: f32,
) -> u32 {
    SCENE.with(|s| {
        u32::from(s.borrow_mut().add_plane(Plane {
            id,
            center: Vec3(x, y, z),
            right: Vec3(rx, ry, rz),
            up: Vec3(ux, uy, uz),
            width: w,
            height: h,
        }))
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_matrix(aspect: f32, fov: f32) -> *const f32 {
    SCENE.with(|s| s.borrow_mut().view_projection(aspect, fov).as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_motion_state() -> u32 {
    SCENE.with(|s| {let s=s.borrow(); u32::from(s.flying) | (u32::from(s.grounded)<<1)})
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_camera() -> *const f32 {
    SCENE.with(|s| {
        let mut s = s.borrow_mut();
        let p = s.position;
        let f = s.forward();
        s.camera = [p.0, p.1, p.2, s.yaw, s.pitch, f.0, f.1, f.2];
        s.camera.as_ptr()
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_spatial_pick() -> *const f32 {
    SCENE.with(|s| {
        let mut s = s.borrow_mut();
        s.hit = s
            .pick(s.position, s.forward())
            .map_or([0.0; 4], |h| [h.id as f32, h.distance, h.u, h.v]);
        s.hit.as_ptr()
    })
}
