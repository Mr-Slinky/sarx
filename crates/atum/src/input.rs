use glam::{Vec2, Vec3};
use amaterasu::Camera;
use zurvan::Simulation;

/// Returns the point on the ground under `cursor`, where the hand aims.
///
/// `cursor` is in pixels from the window's top-left corner, and `window_size` gives the
/// window's width and height in pixels. The camera casts a ray through the cursor, and the
/// returned point is where that ray meets the ground.
///
/// Returns `None` where `cursor` is `None`, as it is before the cursor first moves over the
/// window. Also returns `None` where the camera cannot cast a ray for `window_size`, or where
/// the ray does not meet the ground.
///
/// # Examples
///
/// The cursor in the centre of the window aims the hand at the point that the camera looks at.
///
/// ```ignore
/// // Returns the camera's target, at a height of zero
/// let camera = Camera::new(App::CAMERA_EYE, App::CAMERA_TARGET);
/// let window_size = Vec2::new(800.0, 600.0);
/// let cursor = Some(window_size / 2.0);
/// let ground_point = input::find_hand_target(cursor, window_size, &camera);
/// ```
pub(crate) fn find_hand_target(cursor: Option<Vec2>, window_size: Vec2, camera: &Camera) -> Option<Vec3> {
    let ray = camera.cast_ray(cursor?, window_size)?;
    ray.meet_height(Simulation::HOVER_HEIGHT)
}
