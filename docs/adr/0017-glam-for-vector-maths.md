---
status: accepted
date: 2026-11-10
decision-makers: Kheagen Haskins
---

# glam for Vector Maths

## Context and Problem Statement

The hand step draws the god's hand in 3D, and moves it with the cursor. Both parts of that step need
vector and matrix maths.

The camera gives the vertex shader two matrices. The view matrix moves every point in the world so
that the camera sits at the origin, looking down one axis. The projection matrix then applies
perspective, which makes distant points move towards the centre of the screen.

The ray from the cursor runs the same maths in reverse. It starts from a point on the screen, and
undoes the projection and the view to find a line in the world. That reverse step needs the inverse
of each matrix.

The light also needs vectors. The shader compares the normal of each triangle with the direction of
the light.

The Rust standard library has no vector or matrix types. Later layers add to the need: skinning
blends bone matrices, the poses in ADR 0014 turn each bone by slerp, and the physics engine in ADR
0010 moves bodies through space.

Where does Sarx get its vector and matrix maths?

## Decision Drivers

- The README sets three questions for every library. They ask whether it brings its own
  architecture, what it hides, and how much work it does.
- Under the second goal in the README, the developer means to understand the graphics pipeline. The
  lesson of the camera lies in which matrices it builds, and in the order that the shader applies
  them.
- Every layer from the hand step onwards uses this maths. The camera, the ray, skinning, the poses
  and the physics engine all depend on it.
- WGSL, the shader language, stores a 4×4 matrix as four columns of four `f32` values. A matrix that
  the CPU sends to the shader must match that layout.

## Considered Options

- glam
- Maths written by hand
- nalgebra

## Decision Outcome

Chosen option: "glam", because it answers all three questions in the README in its favour. It brings
no architecture, it hides only arithmetic, and it saves work in every layer that follows.

glam supplies the types that the hand step uses: `Vec3` for points, directions and normals, `Mat4`
for the camera, and `Quat` for the rotations that ADR 0014 turns by slerp.

The developer still builds each matrix, and decides what it contains. For example, the camera calls
`Mat4::look_at_rh` for the view matrix and `Mat4::perspective_rh` for the projection matrix. The
developer chooses where the camera stands, what it looks at, how wide it sees, and the order that
the shader multiplies the matrices in. glam performs the multiplication.

`Mat4::perspective_rh` maps depth to the range from 0 to 1. wgpu uses the same range for its depth
buffer. glam also offers `Mat4::perspective_rh_gl`, which maps depth from -1 to 1 for OpenGL. Sarx
must use the version without `_gl`.

### Consequences

- Good, because glam stores `Mat4` as four columns, which matches the layout that WGSL expects.
- Good, because glam brings plain types and functions. The design of Sarx stays as it is.
- Good, because the camera, the ray, skinning, the poses and the physics engine all share one set of
  types.
- Good, because glam offers a `bytemuck` feature. With that feature, a `Mat4` turns into bytes for a
  GPU buffer in the way that ADR 0018 sets out.
- Bad, because glam hides the arithmetic inside each operation, such as how a matrix is inverted.
- Bad, because glam is below version 1.0. A change in its second version number can break the API,
  as with winit. An agent therefore checks the glam source for the version in `Cargo.lock`, as it
  does for wgpu.

### Confirmation

Nothing in the code embodies this decision yet. Once the camera exists, a reader confirms it with two
checks:

- The workspace depends on `glam`.
- Sarx declares no vector or matrix type of its own.

## Pros and Cons of the Options

### glam

glam is a library of vector, matrix and quaternion types for 2D and 3D graphics. It uses SIMD
instructions, which work on four numbers at once, where the CPU supports them.

- Good, because its types match the shapes that graphics code uses, and nothing more.
- Good, because it stores matrices in the column order that WGSL expects.
- Bad, because it covers only the sizes that graphics code uses, up to 4×4.

### Maths Written by Hand

The developer would write `Vec3`, `Mat4` and `Quat`, along with the operations that each step needs.

- Good, because the developer would write the arithmetic of every operation.
- Bad, because every layer from the hand step onwards would wait on that maths.
- Bad, because a mistake in an inverse or a slerp shows up only as a wrong picture on the screen. It
  gives no error that points to the faulty line.

### nalgebra

nalgebra is a library of general linear algebra, with matrices of any size.

- Good, because it covers far more maths than graphics code needs, such as solving systems of
  equations.
- Bad, because its generic types make its signatures long and its error messages hard to read.
- Bad, because Sarx needs only the small fixed sizes that glam already offers.
