use std::f32::consts::FRAC_PI_4;
use glam::{Mat4, Vec2, Vec3};
use glam::camera::rh::{proj, view};

pub struct Camera {
    eye: Vec3,    // where the camera is positioned, in world units
    target: Vec3, // the point that the camera looks at
}

pub struct Ray {
    origin: Vec3,    // where the ray starts, on the camera's near plane
    direction: Vec3, // the way the ray points, one unit long
}

impl Camera {
    // How much the camera sees from the bottom of the view to the top (45 degrees, in radians)
    const FOV_Y: f32 = FRAC_PI_4;
    // The nearest distance that the camera draws
    const NEAR: f32 = 0.1;
    // The furthest distance that the camera draws
    const FAR: f32 = 100.0;

    pub fn new(eye: Vec3, target: Vec3) -> Self {
        // Stores where the camera stands and what it looks at
        Self { eye, target }
    }

    pub fn build_view_projection(&self, aspect_ratio: f32) -> Mat4 {
        let view_matrix = view::look_at_mat4(self.eye, self.target, Vec3::Y);
        // applies perspective and maps depth to 0 to 1
        let proj_matrix = proj::directx::perspective(Self::FOV_Y, aspect_ratio, Self::NEAR, Self::FAR);

        // applies the view first and the projection second
        proj_matrix * view_matrix
    }

    /// The hand must follow the cursor. The cursor, however, is a point on the screen, and
    /// the hand lives in the world. One pixel covers a whole line of world points, from
    /// just in front of the camera out to the furthest distance it draws. This method finds
    /// that line.
    pub fn cast_ray(&self, cursor: Vec2, window_size: Vec2) -> Option<Ray> {
        // Returns `None` for a window with no area, such as a minimised one, since the maths divides by its size
        if window_size.x <= 0.0 || window_size.y <= 0.0 {
            return None;
        }

        // Turns the cursor's x from pixels into the range -1 to 1, as clip space uses
        let clip_x = 2.0 * cursor.x / window_size.x - 1.0;
        // Does the same for y, flipped, since pixel y grows downwards and clip space y grows upwards
        let clip_y = 1.0 - 2.0 * cursor.y / window_size.y;
        let aspect_ratio = window_size.x / window_size.y;
        // Builds the camera's matrix for the above aspect ratio
        let view_projection = self.build_view_projection(aspect_ratio);
        // Inverts it, which gives a matrix that turns clip space back into world positions
        let inverse = view_projection.inverse();
        // Finds the world point under the cursor at the near edge of the view, at depth 0
        let near_point = inverse.project_point3(Vec3::new(clip_x, clip_y, 0.0));
        // Finds the world point under the cursor at the far edge of the view, at depth 1
        let far_point = inverse.project_point3(Vec3::new(clip_x, clip_y, 1.0));
        // Points from the near point towards the far point, at a length of one unit
        let direction = (far_point - near_point).normalize();
        // Returns the ray
        Some(Ray {
            origin: near_point,
            direction,
        })
    }

}

impl Ray {
    pub fn meet_ground(&self) -> Option<Vec3> {
        // Returns `None` where the ray points level or upwards, since it then never reaches the ground
        if self.direction.y >= 0.0 {
            return None;
        }
        // Finds how far along the ray its height falls to zero
        let distance = -self.origin.y / self.direction.y;
        // Returns the point at that distance
        Some(self.origin + self.direction * distance)
    }
}


