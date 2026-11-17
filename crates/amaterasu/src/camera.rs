use glam::camera::rh::{proj, view};
use glam::{Mat4, Vec2, Vec3};
use std::f32::consts::FRAC_PI_4;

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
        Self { eye, target }
    }

    pub fn build_view_projection(&self, aspect_ratio: f32) -> Mat4 {
        let view_matrix = view::look_at_mat4(self.eye, self.target, Vec3::Y);
        // Applies perspective, and maps depth to the range 0 to 1
        let proj_matrix =
            proj::directx::perspective(Self::FOV_Y, aspect_ratio, Self::NEAR, Self::FAR);

        // Applies the view first and the projection second
        proj_matrix * view_matrix
    }

    /// Returns the ray that runs through the world under `cursor`.
    ///
    /// The cursor is a point on the screen, in pixels from the window's top-left corner.
    /// `window_size` gives the window's width and height in pixels. One pixel covers a whole line
    /// of world points, from the camera's near plane out to the furthest distance that the camera
    /// draws. The returned ray starts on the near plane and follows that line, one unit long.
    ///
    /// Returns `None` where the window's width or height is not a finite number above zero, as
    /// with the zero size of a minimised window. It also returns `None` where the camera cannot
    /// build a ray, as when its eye sits on its target, or straight above or below it.
    ///
    /// # Examples
    ///
    /// The example finds the point on the ground under the cursor.
    ///
    /// ```ignore
    /// // Casts the ray under the cursor, then finds where it meets the ground
    /// if let Some(ray) = camera.cast_ray(cursor, window_size)
    ///     && let Some(ground_point) = ray.meet_ground()
    /// {
    ///     println!("The cursor is over {ground_point}");
    /// }
    /// ```
    pub fn cast_ray(&self, cursor: Vec2, window_size: Vec2) -> Option<Ray> {
        // Returns `None` for a window with no usable size, such as a minimised one, since the maths divides by its size.
        // A comparison with NaN is false, so the check asks for a size above zero rather than rejecting one at or below it
        let has_usable_size = window_size.is_finite() && window_size.x > 0.0 && window_size.y > 0.0;
        if !has_usable_size {
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
        // Returns `None` for a ray made of NaN, which a camera gives when it has no way to look at its target
        if !near_point.is_finite() || !direction.is_finite() {
            return None;
        }
        Some(Ray {
            origin: near_point,
            direction,
        })
    }
}

impl Ray {
    pub fn meet_ground(&self) -> Option<Vec3> {
        self.meet_height(0.0)
    }

    pub fn meet_height(&self, height: f32) -> Option<Vec3> {
        // Returns `None` where the ray points level or upwards, since it then never reaches the ground
        if self.direction.y >= 0.0 {
            return None;
        }
        // Finds how far along the ray its height falls to zero
        let distance = (height - self.origin.y) / self.direction.y;
        // Returns `None` where that point lies behind the origin, as it does for a ray that starts below the ground
        if distance < 0.0 {
            return None;
        }
        // Returns the point at that distance
        Some(self.origin + self.direction * distance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The largest difference that a comparison of two positions or directions accepts
    const TOLERANCE: f32 = 1e-4;

    /// The size of the window that most of the tests cast rays through, in pixels
    const WINDOW_SIZE: Vec2 = Vec2::new(800.0, 600.0);

    /// Returns the camera that most of the tests use, five units from the origin and looking at it
    fn build_camera() -> Camera {
        Camera::new(Vec3::new(0.0, 3.0, 4.0), Vec3::ZERO)
    }

    /// Checks that a ray meets the ground only where the crossing lies ahead of its origin
    #[test]
    fn meet_ground_with_origins_and_directions_returns_point_ahead_only() {
        let cases = [
            (
                "above, pointing straight down",
                Vec3::new(1.0, 2.0, 3.0),
                Vec3::NEG_Y,
                Some(Vec3::new(1.0, 0.0, 3.0)),
            ),
            (
                "above, pointing down at an angle",
                Vec3::new(0.0, 4.0, 0.0),
                Vec3::new(0.6, -0.8, 0.0),
                Some(Vec3::new(3.0, 0.0, 0.0)),
            ),
            (
                "on the ground, pointing down",
                Vec3::new(1.0, 0.0, 3.0),
                Vec3::NEG_Y,
                Some(Vec3::new(1.0, 0.0, 3.0)),
            ),
            (
                "above, pointing level",
                Vec3::new(1.0, 2.0, 3.0),
                Vec3::X,
                None,
            ),
            (
                "below, pointing down",
                Vec3::new(1.0, -2.0, 3.0),
                Vec3::NEG_Y,
                None,
            ),
            (
                "above, pointing up",
                Vec3::new(1.0, 2.0, 3.0),
                Vec3::Y,
                None,
            ),
        ];

        for (name, origin, direction, expected) in cases {
            let ray = Ray { origin, direction };

            let result = ray.meet_ground();

            let matches_expected = match (result, expected) {
                (Some(point), Some(expected_point)) => point.abs_diff_eq(expected_point, TOLERANCE),
                (None, None) => true,
                _ => false,
            };
            assert!(
                matches_expected,
                "ground point for a ray {name}: got {result:?}, expected {expected:?}"
            );
        }
    }

    /// Checks that the ray through the centre of the window points from the eye to the target
    #[test]
    fn cast_ray_with_cursor_at_centre_returns_direction_to_target() {
        let camera = build_camera();

        let ray = camera.cast_ray(WINDOW_SIZE / 2.0, WINDOW_SIZE);

        let expected = (camera.target - camera.eye).normalize();
        assert!(
            ray.is_some_and(|ray| ray.direction.abs_diff_eq(expected, TOLERANCE)),
            "direction of the ray through the centre"
        );
    }

    /// Checks that the ray through the centre of the window starts `NEAR` from the eye
    #[test]
    fn cast_ray_with_cursor_at_centre_returns_origin_on_near_plane() {
        let camera = build_camera();

        let ray = camera.cast_ray(WINDOW_SIZE / 2.0, WINDOW_SIZE);

        let forward = (camera.target - camera.eye).normalize();
        let expected = camera.eye + forward * Camera::NEAR;
        assert!(
            ray.is_some_and(|ray| ray.origin.abs_diff_eq(expected, TOLERANCE)),
            "origin of the ray through the centre"
        );
    }

    /// Checks that every ray is one unit long, wherever the cursor is
    #[test]
    fn cast_ray_with_cursor_anywhere_returns_unit_direction() {
        let cases = [
            ("centre", Vec2::new(400.0, 300.0)),
            ("top-left corner", Vec2::ZERO),
            ("bottom-right corner", WINDOW_SIZE),
            ("off-centre point", Vec2::new(123.0, 456.0)),
            ("point outside the window", Vec2::new(-200.0, 900.0)),
        ];
        let camera = build_camera();

        for (name, cursor) in cases {
            let ray = camera.cast_ray(cursor, WINDOW_SIZE);

            assert!(
                ray.is_some_and(|ray| (ray.direction.length() - 1.0).abs() < TOLERANCE),
                "length of the ray through the {name}"
            );
        }
    }

    /// Checks that the ray through each corner of the window runs through the matching corner of
    /// clip space, where x and y each reach -1 or 1
    #[test]
    fn cast_ray_with_cursor_at_corners_returns_rays_through_clip_corners() {
        let cases = [
            ("top-left", Vec2::new(0.0, 0.0), Vec2::new(-1.0, 1.0)),
            ("top-right", Vec2::new(800.0, 0.0), Vec2::new(1.0, 1.0)),
            ("bottom-left", Vec2::new(0.0, 600.0), Vec2::new(-1.0, -1.0)),
            (
                "bottom-right",
                Vec2::new(800.0, 600.0),
                Vec2::new(1.0, -1.0),
            ),
        ];
        let camera = build_camera();
        let view_projection = camera.build_view_projection(WINDOW_SIZE.x / WINDOW_SIZE.y);

        for (name, cursor, expected) in cases {
            let ray = camera.cast_ray(cursor, WINDOW_SIZE);

            // Places a point ten units along the ray, and finds where it lands in clip space
            let clip =
                ray.map(|ray| view_projection.project_point3(ray.origin + ray.direction * 10.0));
            assert!(
                clip.is_some_and(|clip| clip.truncate().abs_diff_eq(expected, TOLERANCE)),
                "clip position along the ray through the {name} corner: got {clip:?}"
            );
        }
    }

    /// Checks that the centre of a window one pixel wide and high still casts the ray to the target
    #[test]
    fn cast_ray_with_one_pixel_window_returns_direction_to_target() {
        let camera = build_camera();

        let ray = camera.cast_ray(Vec2::splat(0.5), Vec2::ONE);

        let expected = (camera.target - camera.eye).normalize();
        assert!(
            ray.is_some_and(|ray| ray.direction.abs_diff_eq(expected, TOLERANCE)),
            "direction of the ray through a one-pixel window"
        );
    }

    /// Checks that a window size that is not a finite number above zero gives no ray
    #[test]
    fn cast_ray_with_unusable_window_size_returns_none() {
        let cases = [
            ("zero width", Vec2::new(0.0, 600.0)),
            ("zero height", Vec2::new(800.0, 0.0)),
            ("negative width", Vec2::new(-800.0, 600.0)),
            ("negative height", Vec2::new(800.0, -600.0)),
            ("NaN width", Vec2::new(f32::NAN, 600.0)),
            ("NaN height", Vec2::new(800.0, f32::NAN)),
            ("infinite width", Vec2::new(f32::INFINITY, 600.0)),
            ("infinite height", Vec2::new(800.0, f32::INFINITY)),
            (
                "negative infinite width",
                Vec2::new(f32::NEG_INFINITY, 600.0),
            ),
        ];
        let camera = build_camera();

        for (name, window_size) in cases {
            let ray = camera.cast_ray(Vec2::ZERO, window_size);

            assert!(ray.is_none(), "ray through a window with {name}");
        }
    }

    /// Checks that a camera with no way to look at its target gives no ray
    #[test]
    fn cast_ray_with_degenerate_camera_returns_none() {
        let cases = [
            (
                "eye on the target",
                Vec3::new(1.0, 2.0, 3.0),
                Vec3::new(1.0, 2.0, 3.0),
            ),
            (
                "eye straight above the target",
                Vec3::new(0.0, 5.0, 0.0),
                Vec3::ZERO,
            ),
        ];

        for (name, eye, target) in cases {
            let camera = Camera::new(eye, target);

            let ray = camera.cast_ray(WINDOW_SIZE / 2.0, WINDOW_SIZE);

            assert!(ray.is_none(), "ray from a camera with its {name}");
        }
    }

    /// Checks that the target lands at the centre of clip space, wherever the eye is
    #[test]
    fn build_view_projection_with_any_eye_projects_target_to_centre() {
        let cases = [
            ("above and in front", Vec3::new(0.0, 3.0, 4.0), Vec3::ZERO),
            ("to one side", Vec3::new(-6.0, 0.5, 0.0), Vec3::ZERO),
            (
                "away from the origin",
                Vec3::new(10.0, 8.0, -3.0),
                Vec3::new(2.0, 1.0, 5.0),
            ),
        ];

        for (name, eye, target) in cases {
            let camera = Camera::new(eye, target);

            let clip = camera.build_view_projection(1.5).project_point3(target);

            assert!(
                clip.truncate().abs_diff_eq(Vec2::ZERO, TOLERANCE),
                "clip position of the target from an eye {name}: got {clip}"
            );
        }
    }

    /// Checks that a point `NEAR` in front of the eye lands at depth 0, and one `FAR` in front at
    /// depth 1
    #[test]
    fn build_view_projection_with_near_and_far_points_maps_depth_zero_and_one() {
        let camera = build_camera();
        let forward = (camera.target - camera.eye).normalize();
        let near_point = camera.eye + forward * Camera::NEAR;
        let far_point = camera.eye + forward * Camera::FAR;

        let view_projection = camera.build_view_projection(1.5);
        let near_depth = view_projection.project_point3(near_point).z;
        let far_depth = view_projection.project_point3(far_point).z;

        assert!(
            near_depth.abs() < TOLERANCE,
            "depth of the near point: got {near_depth}"
        );
        assert!(
            (far_depth - 1.0).abs() < TOLERANCE,
            "depth of the far point: got {far_depth}"
        );
    }

    /// Checks that the top-right corner of the view lands at the top-right corner of clip space,
    /// for windows taller than they are wide and for windows wider than they are tall
    #[test]
    fn build_view_projection_with_aspect_ratios_maps_view_corner_to_clip_corner() {
        let cases = [
            ("tall", 0.5),
            ("square", 1.0),
            ("wide", 16.0 / 9.0),
            ("very wide", 4.0),
        ];
        // Looks from five units along z at the origin, so the plane z = 0 lies five units ahead
        let camera = Camera::new(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO);
        let half_height = (Camera::FOV_Y / 2.0).tan() * 5.0;

        for (name, aspect_ratio) in cases {
            let corner = Vec3::new(half_height * aspect_ratio, half_height, 0.0);

            let clip = camera
                .build_view_projection(aspect_ratio)
                .project_point3(corner);

            assert!(
                clip.truncate().abs_diff_eq(Vec2::ONE, TOLERANCE),
                "clip position of the view corner for a {name} window: got {clip}"
            );
        }
    }
}
