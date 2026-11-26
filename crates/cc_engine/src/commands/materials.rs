//! Materials: create, duplicate, rename, change, delete; the material libraries.

use super::{edit, patch_value};
use crate::error::{EngineError, Result};
use crate::refs::{self, Ref};
use crate::registry::{Command, CommandKind};
use crate::session::Session;
use crate::t;
use cc_core::material_library::{LibraryMaterial, MaterialLibrary};
use cc_core::{Material, Project};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

/// `base`, or `base 2`, `base 3`… when taken.
fn unique_name(p: &Project, base: &str) -> String {
    let base = base.trim();
    let mut name = base.to_string();
    let mut n = 2;
    while p.material(&name).is_some() {
        name = format!("{base} {n}");
        n += 1;
    }
    name
}

fn material_out(s: &Session, name: &str) -> Value {
    s.project().material(name).map(material_json).unwrap_or(json!({ "name": name }))
}

/// A material's fields as JSON, its texture's image as a summary (format,
/// size in pixels and bytes) instead of its bytes.
pub(crate) fn material_json(m: &Material) -> Value {
    let mut v = serde_json::to_value(m).unwrap_or(json!({ "name": m.name }));
    if let (Some(t), Some(slot)) = (&m.appearance.texture, v.pointer_mut("/appearance/texture/image")) {
        *slot = json!({
            "format": t.image.format,
            "width": t.image.width,
            "height": t.image.height,
            "bytes": t.image.data.len(),
        });
    }
    v
}

/// Create a material (default name "Material"; a number is added when taken).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialCreate {
    /// Name (default: "Material"; a number is added when taken).
    #[serde(default)]
    pub name: Option<String>,
    /// Shading color `[r, g, b]`, 0–255 (default light gray).
    #[serde(default)]
    pub color: Option<[u8; 3]>,
}

impl Command for MaterialCreate {
    const NAME: &'static str = "material_create";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let name = unique_name(s.project(), &self.name.unwrap_or_else(|| t!("mat-new-name")));
        let material = Material::new(name.clone(), self.color.unwrap_or([190, 190, 190]));
        edit(s, t!("tx-modify-material"), |e| {
            e.materials_mut().push(material);
            Ok(())
        })?;
        Ok(material_out(s, &name))
    }
}

/// Copy a material under a new name (default "<name> (copy)").
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialDuplicate {
    pub material: Ref,
    /// Name of the copy (default: "<name> (copy)").
    #[serde(default)]
    pub name: Option<String>,
}

impl Command for MaterialDuplicate {
    const NAME: &'static str = "material_duplicate";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let src = p
            .material(&refs::material(p, &self.material)?)
            .cloned()
            .ok_or_else(|| EngineError::not_found("material"))?;
        let name = unique_name(p, &self.name.unwrap_or_else(|| t!("copy-of", "name" => src.name)));
        let copy = Material { name: name.clone(), ..src };
        edit(s, t!("tx-modify-material"), |e| {
            e.materials_mut().push(copy);
            Ok(())
        })?;
        Ok(material_out(s, &name))
    }
}

/// Rename a material; every type using it follows.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialRename {
    pub material: Ref,
    /// New name.
    pub name: String,
}

impl Command for MaterialRename {
    const NAME: &'static str = "material_rename";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let old = refs::material(s.project(), &self.material)?;
        let new = self.name.trim().to_string();
        if new.is_empty() {
            return Err(EngineError::invalid("the name is empty"));
        }
        if new != old {
            edit(s, t!("tx-rename-material"), |e| {
                if e.rename_material(&old, &new) {
                    Ok(())
                } else {
                    Err(EngineError::rejected("the name is already used"))
                }
            })?;
        }
        Ok(material_out(s, &new))
    }
}

/// Change a material's fields (merged into what `material_get` shows:
/// description, class, colors, patterns, transparency, finish, physical
/// values…). Use `material_rename` to rename it.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialSet {
    pub material: Ref,
    /// Fields to change, shaped like the result of `material_get`.
    pub values: Value,
}

impl Command for MaterialSet {
    const NAME: &'static str = "material_set";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let name = refs::material(p, &self.material)?;
        let current = p.material(&name).ok_or_else(|| EngineError::not_found("material"))?;
        let mut changed = patch_value(current, &self.values)?;
        if changed.name != name {
            return Err(EngineError::invalid("use material_rename to rename a material"));
        }
        if changed.repair() > 0 {
            return Err(EngineError::invalid("invalid material values"));
        }
        edit(s, t!("tx-modify-material"), |e| {
            if let Some(m) = e.materials_mut().iter_mut().find(|m| m.name == name) {
                *m = changed;
            }
            Ok(())
        })?;
        Ok(material_out(s, &name))
    }
}

/// Delete a material no type uses.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialDelete {
    pub material: Ref,
}

impl Command for MaterialDelete {
    const NAME: &'static str = "material_delete";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let name = refs::material(p, &self.material)?;
        let users = p.material_users(&name).len();
        if users > 0 {
            return Err(EngineError::rejected(format!("{users} types use this material")));
        }
        let elements = p.material_instance_users(&name).len();
        if elements > 0 {
            return Err(EngineError::rejected(format!("{elements} components use this material")));
        }
        let painted = p.material_paint_users(&name).len();
        if painted > 0 {
            return Err(EngineError::rejected(format!("{painted} elements have faces painted with this material")));
        }
        edit(s, t!("tx-modify-material"), |e| {
            e.materials_mut().retain(|m| m.name != name);
            Ok(())
        })?;
        Ok(json!({ "deleted": name }))
    }
}

/// Give a material an image texture (shown on its faces in the textures
/// visual style of 3D views), or change how its texture is laid out. The
/// image comes from `path` or from `data` (base64); with neither, the
/// current texture's size, offset and rotation change. One repeat covers
/// `width` × `height` meters on the faces: on floors and other horizontal
/// faces along plan X and Y, on walls and slopes level across the face and
/// up it.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialTextureSet {
    pub material: Ref,
    /// Path of a PNG, JPEG, GIF, BMP or WebP file.
    #[serde(default)]
    pub path: Option<std::path::PathBuf>,
    /// The image file's bytes as base64 (instead of `path`).
    #[serde(default)]
    pub data: Option<String>,
    /// Width of one repeat, meters (default: the current one, else 1 m).
    #[serde(default)]
    pub width: Option<f64>,
    /// Height of one repeat, meters (default: the current one, else the
    /// width in the image's proportions).
    #[serde(default)]
    pub height: Option<f64>,
    /// Where a repeat starts, `[x, y]` meters along the face (default: the current one, else `[0, 0]`).
    #[serde(default)]
    pub offset: Option<[f64; 2]>,
    /// Rotation of the image on the faces, degrees counter-clockwise (default: the current one, else 0).
    #[serde(default)]
    pub rotation: Option<f64>,
}

impl Command for MaterialTextureSet {
    const NAME: &'static str = "material_texture_set";
    const KIND: CommandKind = CommandKind::Edit;
    const READS_FILES: bool = true;
    fn run(self, s: &mut Session) -> Result<Value> {
        use cc_core::material::{MAX_TEXTURE_BYTES, MAX_TEXTURE_SIZE, MIN_TEXTURE_SIZE, MaterialTexture};
        let p = s.project();
        let name = refs::material(p, &self.material)?;
        let current = p.material(&name).ok_or_else(|| EngineError::not_found("material"))?.appearance.texture.clone();
        let bytes = match (&self.path, &self.data) {
            (Some(_), Some(_)) => return Err(EngineError::invalid("give either path or data, not both")),
            (Some(path), None) => {
                s.require_read("path", path)?;
                if std::fs::metadata(path)?.len() > MAX_TEXTURE_BYTES as u64 {
                    return Err(EngineError::invalid(t!("err-image-too-large")));
                }
                Some(std::fs::read(path)?)
            }
            (None, Some(text)) => {
                if text.len() / 4 * 3 > MAX_TEXTURE_BYTES + 3 {
                    return Err(EngineError::invalid(t!("err-image-too-large")));
                }
                Some(
                    cc_core::raster::base64_decode(text)
                        .ok_or_else(|| EngineError::invalid("data is not valid base64"))?,
                )
            }
            (None, None) => None,
        };
        let size = |v: Option<f64>, what: &str| -> Result<Option<f64>> {
            v.map(|v| {
                if v.is_finite() && (MIN_TEXTURE_SIZE..=MAX_TEXTURE_SIZE).contains(&v) {
                    Ok(v)
                } else {
                    Err(EngineError::invalid(format!(
                        "{what} must be between {MIN_TEXTURE_SIZE} and {MAX_TEXTURE_SIZE} m"
                    )))
                }
            })
            .transpose()
        };
        let (width, height) = (size(self.width, "width")?, size(self.height, "height")?);
        let offset =
            self.offset.map(|[x, y]| Ok::<_, EngineError>([refs::finite(x, "offset")?, refs::finite(y, "offset")?]));
        let offset = offset.transpose()?;
        let rotation = self.rotation.map(|r| refs::finite(r, "rotation")).transpose()?;
        let mut texture = match bytes {
            Some(bytes) => {
                let image = crate::session::guard(|| cc_views::raster::probe(bytes))?
                    .ok_or_else(|| EngineError::invalid(t!("err-image-unreadable")))?;
                let mut texture = MaterialTexture::new(image, 1.0, 1.0);
                if !texture.image_ok() {
                    return Err(EngineError::invalid(t!("err-image-too-large")));
                }
                if let Some(c) = &current {
                    texture = MaterialTexture { image: texture.image, ..c.clone() };
                }
                texture
            }
            None => current.ok_or_else(|| EngineError::invalid("the material has no texture: give path or data"))?,
        };
        // A new texture without a size of its own keeps the image's proportions.
        let aspect = f64::from(texture.image.height) / f64::from(texture.image.width.max(1));
        match (width, height) {
            (Some(w), Some(h)) => (texture.width, texture.height) = (w, h),
            (Some(w), None) => {
                texture.width = w;
                texture.height = (w * aspect).clamp(MIN_TEXTURE_SIZE, MAX_TEXTURE_SIZE);
            }
            (None, Some(h)) => texture.height = h,
            (None, None) => {}
        }
        if let Some(o) = offset {
            texture.offset = o;
        }
        if let Some(r) = rotation {
            texture.rotation_deg = r;
        }
        if texture.repair() > 0 {
            return Err(EngineError::invalid("invalid texture values"));
        }
        edit(s, t!("tx-modify-material"), |e| {
            if let Some(m) = e.materials_mut().iter_mut().find(|m| m.name == name) {
                m.appearance.texture = Some(texture);
            }
            Ok(())
        })?;
        Ok(material_out(s, &name))
    }
}

/// Take a material's texture away (its faces show the appearance color).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialTextureClear {
    pub material: Ref,
}

impl Command for MaterialTextureClear {
    const NAME: &'static str = "material_texture_clear";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let p = s.project();
        let name = refs::material(p, &self.material)?;
        if p.material(&name).is_none_or(|m| m.appearance.texture.is_none()) {
            return Err(EngineError::invalid("the material has no texture"));
        }
        edit(s, t!("tx-modify-material"), |e| {
            if let Some(m) = e.materials_mut().iter_mut().find(|m| m.name == name) {
                m.appearance.texture = None;
            }
            Ok(())
        })?;
        Ok(material_out(s, &name))
    }
}

/// The name of built-in library `code` in the active language, else `fallback`.
fn library_name(code: &str, fallback: &str) -> String {
    cc_core::material_library::find(code)
        .and_then(|(b, _)| crate::i18n::try_tr(b.name_key))
        .unwrap_or_else(|| fallback.to_string())
}

/// A library's summary: code, name, region, standard, number of materials.
fn library_json(lib: &MaterialLibrary) -> Value {
    json!({
        "code": lib.code,
        "name": library_name(&lib.code, &lib.name),
        "region": lib.region,
        "standard": lib.standard,
        "materials": lib.materials.len(),
    })
}

/// The built-in material libraries (the materials of a region or standard,
/// such as Brazil's structural steels, reinforcing steels and concretes):
/// each one's `code`, translated `name`, `region`, `standard` and number of
/// `materials`. With `library`, that library alone, with its `sources` and
/// its `materials` (name, class, color, description and physical values in
/// SI units: kg/m³, W/(m·K), J/(kg·K), 1/K, MPa), each saying whether the
/// project has a material of any of its names (`in_project`). A material's
/// `name` is the one of `language` (its `main_name` when it has none), and
/// `names` holds its translations.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct MaterialCatalog {
    /// A built-in library's code (ignoring case): list its materials.
    #[serde(default)]
    pub library: Option<String>,
    /// The language of the materials' names, a code such as `en` or `pt-BR`
    /// (default: the active language).
    #[serde(default)]
    pub language: Option<String>,
}

/// The language code a material library command names materials in.
fn names_language(language: &Option<String>) -> String {
    language.as_deref().map(str::trim).filter(|l| !l.is_empty()).map_or_else(
        || crate::i18n::language().code().to_string(),
        |l| crate::i18n::Language::from_code(l).map_or_else(|| l.to_string(), |g| g.code().to_string()),
    )
}

/// Whether the project has a material named any of `m`'s names.
fn in_project(p: &cc_core::Project, m: &LibraryMaterial) -> bool {
    m.all_names().any(|n| p.material(n).is_some())
}

impl Command for MaterialCatalog {
    const NAME: &'static str = "material_catalog";
    const KIND: CommandKind = CommandKind::Query;
    fn run(self, s: &mut Session) -> Result<Value> {
        let Some(code) = &self.library else {
            let all: Vec<Value> = cc_core::material_library::built_in().iter().map(|(_, l)| library_json(l)).collect();
            return Ok(json!({ "libraries": all }));
        };
        let (_, lib) = cc_core::material_library::find(code)
            .ok_or_else(|| EngineError::not_found(format!("no built-in material library '{code}'")))?;
        let p = s.project();
        let lang = names_language(&self.language);
        let materials: Vec<Value> = lib
            .materials
            .iter()
            .map(|m| {
                let mut v = serde_json::to_value(m).unwrap_or(Value::Null);
                if let Some(o) = v.as_object_mut() {
                    o.insert("main_name".into(), json!(m.name));
                    o.insert("name".into(), json!(m.name_in(&lang)));
                    o.insert("in_project".into(), json!(in_project(p, m)));
                }
                v
            })
            .collect();
        let mut out = library_json(lib);
        if let Some(o) = out.as_object_mut() {
            o.insert("sources".into(), json!(lib.sources));
            o.insert("materials".into(), json!(materials));
        }
        Ok(json!({ "libraries": [out] }))
    }
}

/// Import materials from a library into the project: a built-in one by
/// `library` (see `material_catalog`), or a library file (JSON, the format
/// `material_library_export` writes) by `path` or `text`. `materials` picks
/// some by name, in any of their languages (default: all). Materials are
/// named in `language` when the library has their name in it. A material
/// the project already has under any of its names is kept as it is. One
/// undo step; returns the names `imported` and `kept`.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct MaterialLibraryImport {
    /// A built-in library's code (or give `path` or `text`).
    #[serde(default)]
    pub library: Option<String>,
    /// Path of a library file.
    #[serde(default)]
    pub path: Option<PathBuf>,
    /// A library file's text.
    #[serde(default)]
    pub text: Option<String>,
    /// The materials to import, by name (default: all of the library's).
    #[serde(default)]
    pub materials: Option<Vec<String>>,
    /// The language to name them in, a code such as `en` or `pt-BR`
    /// (default: the active language).
    #[serde(default)]
    pub language: Option<String>,
}

/// A library file read for an import, its errors as engine errors.
fn read_library(text: &str) -> Result<MaterialLibrary> {
    cc_core::material_library::read_library(text).map_err(|e| EngineError::invalid(e.to_string()))
}

impl Command for MaterialLibraryImport {
    const NAME: &'static str = "material_library_import";
    const KIND: CommandKind = CommandKind::Edit;
    const READS_FILES: bool = true;
    fn run(self, s: &mut Session) -> Result<Value> {
        use cc_core::material_library::MAX_FILE_BYTES;
        let lib = match (&self.library, &self.path, self.text) {
            (Some(code), None, None) => cc_core::material_library::find(code)
                .map(|(_, l)| l.clone())
                .ok_or_else(|| EngineError::not_found(format!("no built-in material library '{code}'")))?,
            (None, Some(path), None) => {
                s.require_read("path", path)?;
                let len = std::fs::metadata(path)?.len();
                if len > MAX_FILE_BYTES as u64 {
                    return Err(EngineError::invalid(format!("the file is larger than {} MB", MAX_FILE_BYTES >> 20)));
                }
                read_library(&String::from_utf8_lossy(&std::fs::read(path)?))?
            }
            (None, None, Some(text)) => read_library(&text)?,
            _ => return Err(EngineError::invalid("give exactly one of library, path or text")),
        };
        let picked: Vec<&LibraryMaterial> = match &self.materials {
            None => lib.materials.iter().collect(),
            Some(names) => names
                .iter()
                .map(|n| {
                    lib.material(n).ok_or_else(|| EngineError::not_found(format!("no material '{n}' in {}", lib.code)))
                })
                .collect::<Result<_>>()?,
        };
        let p = s.project();
        let lang = names_language(&self.language);
        let (mut new, mut kept): (Vec<Material>, Vec<String>) = (Vec::new(), Vec::new());
        for m in picked {
            let name = m.name_in(&lang);
            if in_project(p, m) || new.iter().any(|x| x.name.eq_ignore_ascii_case(name)) {
                let have = m.all_names().find(|n| p.material(n).is_some()).unwrap_or(name);
                kept.push(have.to_string());
            } else {
                new.push(m.to_material_in(&lang));
            }
        }
        let imported: Vec<String> = new.iter().map(|m| m.name.clone()).collect();
        if !new.is_empty() {
            edit(s, t!("tx-import-materials"), |e| {
                e.materials_mut().extend(new);
                Ok(())
            })?;
        }
        Ok(json!({ "library": lib.code, "imported": imported, "kept": kept }))
    }
}

/// Write project materials as a library file (JSON, SI units) that
/// `material_library_import` reads, for another project or another user:
/// to `path` when given; the `text` is returned either way. `materials`
/// picks them by name (default: all of the project's).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialLibraryExport {
    /// The library's code, as importing projects see it.
    pub code: String,
    /// The library's name.
    pub name: String,
    /// The region it is for: `brazil`, `europe`, `us`, or empty.
    #[serde(default)]
    pub region: String,
    /// The standards its values follow.
    #[serde(default)]
    pub standard: String,
    /// The documents its values come from.
    #[serde(default)]
    pub sources: Vec<String>,
    /// The materials to write, by name (default: all of the project's).
    #[serde(default)]
    pub materials: Option<Vec<String>>,
    /// Path of the file to write.
    #[serde(default)]
    pub path: Option<PathBuf>,
}

impl Command for MaterialLibraryExport {
    const NAME: &'static str = "material_library_export";
    const KIND: CommandKind = CommandKind::File;
    const CHECKS_PATHS: bool = true;
    fn run(self, s: &mut Session) -> Result<Value> {
        if let Some(path) = &self.path {
            s.require_write("path", path)?;
        }
        let p = s.project();
        let materials: Vec<LibraryMaterial> = match &self.materials {
            None => p.materials.iter().map(LibraryMaterial::of_material).collect(),
            Some(names) => names
                .iter()
                .map(|n| {
                    p.material(n)
                        .map(LibraryMaterial::of_material)
                        .ok_or_else(|| EngineError::not_found(format!("material '{n}'")))
                })
                .collect::<Result<_>>()?,
        };
        let lib = MaterialLibrary {
            format: cc_core::material_library::FILE_FORMAT,
            code: self.code,
            name: self.name,
            region: self.region,
            standard: self.standard,
            sources: self.sources,
            materials,
        };
        let text = cc_core::material_library::write_library(&lib);
        // What is written must read back: a bad code, name or value is refused here.
        read_library(&text)?;
        if let Some(path) = &self.path {
            std::fs::write(path, &text)?;
        }
        Ok(json!({ "code": lib.code, "materials": lib.materials.len(), "text": text }))
    }
}
