use crate::components::{Position, Speed, Target, Velocity};
use enki::World;

pub(crate) fn movement(world: &mut World, step: f32) {
    for (position, velocity) in world.query_mut::<(&mut Position, &Velocity)>() {
        position.0 += velocity.0 * step;
    }
}

pub(crate) fn steering(world: &mut World, step: f32) {
    for (velocity, position, target, speed) in
        world.query_mut::<(&mut Velocity, &Position, &Target, &Speed)>()
    {
        let offset = target.0 - position.0;
        velocity.0 = (offset / step).clamp_length_max(speed.0);
    }
}
