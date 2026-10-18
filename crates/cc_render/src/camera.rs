use cc_core::CameraState;
use cc_geom::Ray;
use glam::{DVec3, Mat4, Vec3};

const FOV_Y: f32 = 45.0;

fn eye_target(cam: &CameraState) -> (Vec3, Vec3) {
    (cam.eye().as_vec3(), cam.target.as_vec3())
}

/// Perspective view-projection matrix (Z up, wgpu depth range 0..1).
pub fn view_proj(cam: &CameraState, aspect: f32) -> Mat4 {
    let (eye, target) = eye_target(cam);
    let near = (cam.distance as f32 * 0.01).clamp(0.01, 1.0);
    let proj = glam::camera::rh::proj::directx::perspective(FOV_Y.to_radians(), aspect.max(1e-3), near, 5000.0);
    proj * glam::camera::rh::view::look_at_mat4(eye, target, Vec3::Z)
}

/// World-space ray through normalized device coordinates (`x`, `y` in -1..1, y up).
pub fn pick_ray(cam: &CameraState, aspect: f32, ndc_x: f32, ndc_y: f32) -> Ray {
    let inv = view_proj(cam, aspect).inverse();
    let near = inv.project_point3(Vec3::new(ndc_x, ndc_y, 0.0));
    let far = inv.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));
    let origin: DVec3 = near.as_dvec3();
    Ray { origin, dir: (far.as_dvec3() - origin).normalize() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_ray_hits_target() {
        let cam = CameraState::default();
        let ray = pick_ray(&cam, 1.5, 0.0, 0.0);
        let to_target = (cam.target - ray.origin).normalize();
        assert!(ray.dir.dot(to_target) > 0.9999);
    }
}
