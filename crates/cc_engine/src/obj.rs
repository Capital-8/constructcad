//! Wavefront OBJ models as component types: each object (`o`, or `g` in a
//! file without objects) becomes a type of fixed mesh pieces, one per
//! material of its `.mtl` library, placed by an element at the center of
//! its footprint and the bottom of its extent. OBJ has no units nor a fixed
//! up axis, so the caller gives both.
//!
//! Files are hostile input: unknown statements are skipped, faces with a
//! bad index are dropped, and the sizes read are bounded.

use crate::skp::{Converted, MeshBuild, SkpPlacement, SkpType, mesh_definition};
use glam::{DVec2, DVec3};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// Largest .obj file read, in bytes.
pub const MAX_BYTES: u64 = 512 * 1024 * 1024;
/// Largest .mtl file read, in bytes.
const MAX_MTL_BYTES: u64 = 16 * 1024 * 1024;
/// Vertices read from a file.
const MAX_VERTICES: usize = 10_000_000;
/// Corners of one face.
const MAX_CORNERS: usize = 4_096;
/// Objects (types) an import makes.
const MAX_OBJECTS: usize = 20_000;
/// Faces bent more than this (in meters) out of their plane are split into triangles.
const PLANE_TOLERANCE: f64 = 1e-6;

/// How the file's coordinates map to the model's.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    /// Meters per file unit.
    pub scale: f64,
    /// The file's Y axis points up (the usual OBJ convention); else Z does.
    pub y_up: bool,
}

#[derive(Debug)]
pub enum ObjError {
    /// More vertices, faces or objects than an import takes.
    TooLarge,
}

/// One object of the file: its faces as vertex indices, by material.
struct Object {
    name: String,
    faces: Vec<(Option<usize>, Vec<u32>)>,
}

/// The library statements and material uses of a file, before materials are read.
struct Parsed {
    vertices: Vec<Option<DVec3>>,
    objects: Vec<Object>,
    libraries: Vec<String>,
    /// Material names in the order first used.
    used: Vec<String>,
}

/// Read the statements of `text` (with `\` line continuations).
fn statements(text: &str) -> impl Iterator<Item = String> + '_ {
    let mut lines = text.lines();
    std::iter::from_fn(move || {
        let mut out = String::new();
        loop {
            let line = lines.next()?;
            let line = line.split('#').next().unwrap_or_default();
            match line.trim_end().strip_suffix('\\') {
                Some(head) => {
                    out.push_str(head);
                    out.push(' ');
                }
                None => {
                    out.push_str(line);
                    return Some(out);
                }
            }
        }
    })
}

/// A face corner's vertex index (`v`, `v/vt`, `v//vn` or `v/vt/vn`; negative counts back).
fn corner(token: &str, count: usize) -> Option<u32> {
    let v: i64 = token.split('/').next()?.parse().ok()?;
    let index = match v {
        0 => return None,
        v if v > 0 => v - 1,
        v => count as i64 + v,
    };
    u32::try_from(index).ok().filter(|i| (*i as usize) < count)
}

fn parse(text: &str, frame: Frame) -> Result<Parsed, ObjError> {
    let groups_split = !statements(text).any(|s| s.split_whitespace().next() == Some("o"));
    let mut out = Parsed { vertices: Vec::new(), objects: Vec::new(), libraries: Vec::new(), used: Vec::new() };
    let mut used: HashMap<String, usize> = HashMap::new();
    let mut material: Option<usize> = None;
    let mut faces = 0usize;
    let mut current = Object { name: String::new(), faces: Vec::new() };
    for statement in statements(text) {
        let statement = statement.trim();
        let (keyword, rest) = statement.split_once(char::is_whitespace).unwrap_or((statement, ""));
        let rest = rest.trim();
        match keyword {
            "v" => {
                if out.vertices.len() >= MAX_VERTICES {
                    return Err(ObjError::TooLarge);
                }
                let mut values = rest.split_whitespace().map(|t| t.parse::<f64>().ok());
                let point = match (values.next(), values.next(), values.next()) {
                    (Some(Some(x)), Some(Some(y)), Some(Some(z))) => {
                        let p = if frame.y_up { DVec3::new(x, -z, y) } else { DVec3::new(x, y, z) };
                        Some(p * frame.scale).filter(|p| p.is_finite())
                    }
                    _ => None,
                };
                out.vertices.push(point);
            }
            "f" => {
                let count = out.vertices.len();
                let face: Option<Vec<u32>> =
                    rest.split_whitespace().take(MAX_CORNERS).map(|t| corner(t, count)).collect();
                let Some(face) = face.filter(|f| f.len() >= 3) else { continue };
                faces += 1;
                if faces > crate::skp::MAX_FACES {
                    return Err(ObjError::TooLarge);
                }
                current.faces.push((material, face));
            }
            "o" | "g" if keyword == "o" || groups_split => {
                let name = rest.to_string();
                let done = std::mem::replace(&mut current, Object { name, faces: Vec::new() });
                if !done.faces.is_empty() {
                    if out.objects.len() >= MAX_OBJECTS {
                        return Err(ObjError::TooLarge);
                    }
                    out.objects.push(done);
                }
            }
            "usemtl" => {
                material = (!rest.is_empty()).then(|| {
                    let next = out.used.len();
                    *used.entry(rest.to_string()).or_insert_with(|| {
                        out.used.push(rest.to_string());
                        next
                    })
                });
            }
            "mtllib" if !rest.is_empty() && out.libraries.len() < 16 => out.libraries.push(rest.to_string()),
            _ => {}
        }
    }
    if !current.faces.is_empty() {
        if out.objects.len() >= MAX_OBJECTS {
            return Err(ObjError::TooLarge);
        }
        out.objects.push(current);
    }
    Ok(out)
}

/// The diffuse colors (`Kd`, 0 to 1) of a material library's materials.
pub fn read_mtl(text: &str) -> HashMap<String, [u8; 3]> {
    let mut out = HashMap::new();
    let mut name: Option<String> = None;
    for statement in statements(text) {
        let statement = statement.trim();
        let (keyword, rest) = statement.split_once(char::is_whitespace).unwrap_or((statement, ""));
        match keyword {
            "newmtl" => {
                let n = rest.trim().to_string();
                out.entry(n.clone()).or_insert([200, 200, 200]);
                name = Some(n);
            }
            "Kd" => {
                let Some(n) = &name else { continue };
                let values: Vec<f64> = rest.split_whitespace().filter_map(|t| t.parse().ok()).collect();
                if let [r, g, b, ..] = values[..] {
                    let byte = |v: f64| if v.is_finite() { (v.clamp(0.0, 1.0) * 255.0).round() as u8 } else { 200 };
                    out.insert(n.clone(), [byte(r), byte(g), byte(b)]);
                }
            }
            _ => {}
        }
    }
    out
}

/// The material libraries a file names, looked for by file name only in
/// `dir` (a file must not reach elsewhere on the disk).
fn libraries(dir: Option<&Path>, names: &[String]) -> HashMap<String, [u8; 3]> {
    let mut out = HashMap::new();
    let Some(dir) = dir else { return out };
    // A name may hold spaces; else the statement lists several files.
    let files: std::collections::BTreeSet<&std::ffi::OsStr> = names
        .iter()
        .flat_map(|n| std::iter::once(n.as_str()).chain(n.split_whitespace()))
        .filter_map(|n| Path::new(n).file_name())
        .collect();
    for file in files {
        let path = dir.join(file);
        let small = std::fs::metadata(&path).is_ok_and(|m| m.is_file() && m.len() <= MAX_MTL_BYTES);
        let Some(bytes) = small.then(|| std::fs::read(&path).ok()).flatten() else { continue };
        for (k, v) in read_mtl(&String::from_utf8_lossy(&bytes)) {
            out.entry(k).or_insert(v);
        }
    }
    out
}

/// Convert the OBJ `text` (its material libraries looked for in `dir`) to
/// types and placements; `merge` makes the whole file one type at the origin
/// of the file, named `name`.
pub fn convert(text: &str, dir: Option<&Path>, frame: Frame, name: &str, merge: bool) -> Result<Converted, ObjError> {
    let mut parsed = parse(text, frame)?;
    let colors = libraries(dir, &parsed.libraries);
    let mut out = Converted {
        materials: parsed.used.iter().map(|n| (n.clone(), colors.get(n).copied().unwrap_or([200, 200, 200]))).collect(),
        ..Default::default()
    };
    if merge && parsed.objects.len() > 1 {
        let faces = parsed.objects.drain(..).flat_map(|o| o.faces).collect();
        parsed.objects.push(Object { name: String::new(), faces });
    }
    let single = parsed.objects.len() == 1;
    for (i, object) in parsed.objects.iter().enumerate() {
        let points = |face: &[u32]| -> Option<Vec<DVec3>> {
            face.iter().map(|&i| parsed.vertices.get(i as usize).copied().flatten()).collect()
        };
        // Insertion point: the center of the footprint, at the bottom (the file's origin when merged).
        let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
        for (_, face) in &object.faces {
            for p in points(face).unwrap_or_default() {
                (lo, hi) = (lo.min(p), hi.max(p));
            }
        }
        if !lo.is_finite() || !hi.is_finite() {
            continue;
        }
        let origin = if merge { DVec3::ZERO } else { DVec3::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0, lo.z) };
        let mut pieces: BTreeMap<Option<usize>, MeshBuild> = BTreeMap::new();
        for (material, face) in &object.faces {
            let Some(face) = points(face) else { continue };
            let face: Vec<DVec3> = face.iter().map(|p| *p - origin).collect();
            let mesh = pieces.entry(*material).or_default();
            for polygon in planar_pieces(&face) {
                out.faces += 1;
                mesh.add(&polygon);
            }
        }
        let type_name = match object.name.trim() {
            n if !n.is_empty() => n.to_string(),
            _ if single || merge => name.to_string(),
            _ => format!("{name} {}", i + 1),
        };
        let Some(def) = mesh_definition(&type_name, pieces, &out.materials) else { continue };
        out.types.push(SkpType { name: type_name, def });
        out.placements.push(SkpPlacement {
            ty: out.types.len() - 1,
            position: DVec2::new(origin.x, origin.y),
            z: origin.z,
            rotation_deg: 0.0,
        });
    }
    Ok(out)
}

/// `face` as it is when flat, else as a fan of triangles; nothing when it has no area.
fn planar_pieces(face: &[DVec3]) -> Vec<Vec<DVec3>> {
    // Newell's normal.
    let mut n = DVec3::ZERO;
    for (i, a) in face.iter().enumerate() {
        let b = face[(i + 1) % face.len()];
        n += DVec3::new((a.y - b.y) * (a.z + b.z), (a.z - b.z) * (a.x + b.x), (a.x - b.x) * (a.y + b.y));
    }
    let Some(normal) = n.try_normalize() else { return Vec::new() };
    let base = face[0];
    if face.len() == 3 || face.iter().all(|p| (*p - base).dot(normal).abs() <= PLANE_TOLERANCE) {
        return vec![face.to_vec()];
    }
    face.windows(2)
        .skip(1)
        .map(|w| vec![base, w[0], w[1]])
        .filter(|t| (t[1] - t[0]).cross(t[2] - t[0]).length_squared() > 0.0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_count_from_one_and_back() {
        assert_eq!(corner("1", 3), Some(0));
        assert_eq!(corner("3/7/2", 3), Some(2));
        assert_eq!(corner("-1//4", 3), Some(2));
        assert_eq!(corner("4", 3), None);
        assert_eq!(corner("0", 3), None);
        assert_eq!(corner("-4", 3), None);
        assert_eq!(corner("x", 3), None);
    }

    #[test]
    fn bent_faces_become_triangles() {
        let flat = [DVec3::ZERO, DVec3::X, DVec3::new(1.0, 1.0, 0.0), DVec3::Y];
        assert_eq!(planar_pieces(&flat).len(), 1);
        let bent = [DVec3::ZERO, DVec3::X, DVec3::new(1.0, 1.0, 0.5), DVec3::Y];
        assert_eq!(planar_pieces(&bent).len(), 2);
        assert!(planar_pieces(&[DVec3::ZERO, DVec3::X, DVec3::X * 2.0]).is_empty());
    }

    #[test]
    fn colors_come_from_kd() {
        let colors = read_mtl("newmtl Red\nKd 1 0 0\nnewmtl Plain\nKd nan 1\n");
        assert_eq!(colors.get("Red"), Some(&[255, 0, 0]));
        assert_eq!(colors.get("Plain"), Some(&[200, 200, 200]));
    }
}
