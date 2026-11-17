use glam::{Vec2, Vec3};
use amaterasu::Camera;
use zurvan::Simulation;

/// Returns the point under `cursor` at the hand's hover height, where the hand aims.
///
/// `cursor` is in pixels from the window's top-left corner, and `window_size` gives the
/// window's width and height in pixels. The camera casts a ray through the cursor, and the
/// returned point is where that ray falls to [`Simulation::HOVER_HEIGHT`]. The point therefore
/// sits under the cursor on screen, whatever angle the camera looks from.
///
/// Returns `None` where `cursor` is `None`, as it is before the cursor first moves over the
/// window. Also returns `None` where the camera cannot cast a ray for `window_size`, or where
/// the ray never falls to the hover height.
///
/// # Examples
///
/// The cursor in the centre of the window aims the hand at the point where the camera's line of
/// sight passes the hover height.
///
/// ```ignore
/// // Returns the point on the camera's line of sight, at the hover height
/// let camera = Camera::new(App::CAMERA_EYE, App::CAMERA_TARGET);
/// let window_size = Vec2::new(800.0, 600.0);
/// let cursor = Some(window_size / 2.0);
/// let hover_point = input::find_hand_target(cursor, window_size, &camera);
/// ```
pub(crate) fn find_hand_target(cursor: Option<Vec2>, window_size: Vec2, camera: &Camera) -> Option<Vec3> {
    let ray = camera.cast_ray(cursor?, window_size)?;
    ray.meet_height(Simulation::HOVER_HEIGHT)
}
