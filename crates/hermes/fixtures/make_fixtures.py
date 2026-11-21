"""Writes the triangle, the cube and the broken files that the hermes tests read.

Each fixture is a binary glTF file whose values the script sets by hand, so that a test knows them
before any code reads the file. The script writes the files beside itself:

    python crates/hermes/fixtures/make_fixtures.py

The frozen hand, hand.glb, is a copy of a Blender export, and the script leaves it alone.
"""
import json
import struct
from pathlib import Path

TRIANGLE_POSITIONS = [(1.0, 2.0, 3.0), (4.0, 6.0, 5.0), (9.0, 7.0, 8.0)]
TRIANGLE_NORMALS = [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0)]


def pad(data: bytes, fill: bytes) -> bytes:
    return data + fill * ((4 - len(data) % 4) % 4)


def pack_glb(doc, blob):
    json_chunk = pad(json.dumps(doc, separators=(",", ":")).encode(), b" ")
    out = b""
    out += struct.pack("<I4s", len(json_chunk), b"JSON") + json_chunk
    if blob is not None:
        bin_chunk = pad(blob, b"\x00")
        out += struct.pack("<I4s", len(bin_chunk), b"BIN\x00") + bin_chunk
    return struct.pack("<4sII", b"glTF", 2, 12 + len(out)) + out


def build_mesh(positions, normals, indices, index_format="H", index_type=5123):
    """Returns the bytes of a GLB file with one mesh. Pass None to leave a list out."""
    views, accessors, attributes = [], [], {}
    blob = b""

    def add(data, target, accessor):
        nonlocal blob
        blob = pad(blob, b"\x00")
        views.append({"buffer": 0, "byteOffset": len(blob), "byteLength": len(data), "target": target})
        blob += data
        accessor["bufferView"] = len(views) - 1
        accessors.append(accessor)
        return len(accessors) - 1

    if positions is not None:
        data = b"".join(struct.pack("<3f", *p) for p in positions)
        attributes["POSITION"] = add(data, 34962, {
            "componentType": 5126, "count": len(positions), "type": "VEC3",
            "min": [min(p[k] for p in positions) for k in range(3)],
            "max": [max(p[k] for p in positions) for k in range(3)],
        })
    if normals is not None:
        data = b"".join(struct.pack("<3f", *n) for n in normals)
        attributes["NORMAL"] = add(data, 34962, {"componentType": 5126, "count": len(normals), "type": "VEC3"})
    primitive = {"attributes": attributes}
    if indices is not None:
        data = b"".join(struct.pack("<" + index_format, i) for i in indices)
        primitive["indices"] = add(data, 34963, {"componentType": index_type, "count": len(indices), "type": "SCALAR"})

    doc = {
        "asset": {"version": "2.0", "generator": "sarx hermes test fixture"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [primitive]}],
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [{"byteLength": len(blob)}],
    }
    return pack_glb(doc, blob)


def build_cube():
    x, y, z = (1, 0, 0), (0, 1, 0), (0, 0, 1)
    neg = lambda v: tuple(-c for c in v)
    # (normal, u, v) with u cross v == normal, so each face winds counter-clockwise from outside
    faces = [(x, y, z), (neg(x), z, y), (y, z, x), (neg(y), x, z), (z, x, y), (neg(z), y, x)]
    positions, normals, indices = [], [], []
    for n, u, v in faces:
        base = len(positions)
        for su, sv in ((-1, -1), (1, -1), (1, 1), (-1, 1)):
            positions.append(tuple(float(n[k] + su * u[k] + sv * v[k]) for k in range(3)))
            normals.append(tuple(float(c) for c in n))
        indices += [base, base + 1, base + 2, base, base + 2, base + 3]
    return build_mesh(positions, normals, indices, "I", 5125)


def build_without_binary():
    doc = {"asset": {"version": "2.0", "generator": "sarx hermes test fixture"}}
    return pack_glb(doc, None)


def build_without_mesh():
    doc = {"asset": {"version": "2.0", "generator": "sarx hermes test fixture"}, "buffers": [{"byteLength": 4}]}
    return pack_glb(doc, b"\x00" * 4)


FIXTURES = {
    "triangle.glb": lambda: build_mesh(TRIANGLE_POSITIONS, TRIANGLE_NORMALS, [0, 1, 2]),
    "cube.glb": build_cube,
    "broken/without-binary.glb": build_without_binary,
    "broken/without-mesh.glb": build_without_mesh,
    "broken/without-normals.glb": lambda: build_mesh(TRIANGLE_POSITIONS, None, [0, 1, 2]),
    "broken/without-indices.glb": lambda: build_mesh(TRIANGLE_POSITIONS, TRIANGLE_NORMALS, None),
    "broken/too-few-normals.glb": lambda: build_mesh(TRIANGLE_POSITIONS, TRIANGLE_NORMALS[:2], [0, 1, 2]),
    "broken/index-past-last-vertex.glb": lambda: build_mesh(TRIANGLE_POSITIONS, TRIANGLE_NORMALS, [0, 1, 3]),
}

if __name__ == "__main__":
    out = Path(__file__).parent
    (out / "broken").mkdir(parents=True, exist_ok=True)
    for name, build in FIXTURES.items():
        (out / name).write_bytes(build())
