//! Column and beam types by structural material (concrete, steel, timber):
//! the first type of a material, else a default made from the profile
//! catalog (steel `W200x26,6`, timber `MLC 9x32`) or a 20 x 40 concrete
//! rectangle, in one undo step.

use super::profiles::{ProfileCreate, ProfileImport, ProfileTypeCreate};
use crate::error::{EngineError, Result};
use crate::refs::{self, Ref};
use crate::registry::{Command, CommandKind};
use crate::session::Session;
use crate::t;
use cc_core::{Category, ElementTypeId, ProfileMaterial, ProfileShape, StructuralMaterial};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The catalog row of the steel default: (series, designation).
const STEEL_DEFAULT: (&str, &str) = ("W", "W200x26,6");
/// The catalog row of the timber default.
const TIMBER_DEFAULT: (&str, &str) = ("Glulam", "MLC 9x32");
/// The name of the concrete default profile, a 0.20 x 0.40 rectangle.
const CONCRETE_DEFAULT: &str = "20x40";

pub(crate) fn material_arg(code: &str) -> Result<StructuralMaterial> {
    StructuralMaterial::from_code(code).ok_or_else(|| {
        let codes: Vec<&str> = StructuralMaterial::ALL.iter().map(|m| m.code()).collect();
        EngineError::invalid(format!("unknown structural material '{code}' ({})", codes.join(", ")))
    })
}

pub(crate) fn member_category(code: &str) -> Result<Category> {
    match refs::category(code)? {
        c @ (Category::Column | Category::Beam) => Ok(c),
        c => Err(EngineError::invalid(format!("a column or beam, not {}", refs::category_name(c)))),
    }
}

/// The type of `category` new `material` members get: the first one of the
/// material, else a default made in one undo step. Returns it and whether it was made.
pub(crate) fn structural_type(
    s: &mut Session,
    category: Category,
    material: StructuralMaterial,
) -> Result<(ElementTypeId, bool)> {
    if let Some(t) = s.project().types_of_material(category, material).next() {
        return Ok((t.id, false));
    }
    let depth = s.doc.undo_depth();
    match make_default(s, category, material) {
        Ok(id) => {
            s.doc.squash_since(depth, t!("tx-structural-type"));
            Ok((id, true))
        }
        Err(e) => {
            s.undo_to(depth);
            Err(e)
        }
    }
}

fn make_default(s: &mut Session, category: Category, material: StructuralMaterial) -> Result<ElementTypeId> {
    let profile = match material {
        StructuralMaterial::Steel => catalog_profile(s, STEEL_DEFAULT, material)?,
        StructuralMaterial::Timber => catalog_profile(s, TIMBER_DEFAULT, material)?,
        StructuralMaterial::Concrete => concrete_profile(s)?,
    };
    let category = if category == Category::Column { "column" } else { "beam" };
    let out =
        ProfileTypeCreate { profile: Ref::Name(profile), category: category.into(), ..Default::default() }.run(s)?;
    out["id"].as_u64().map(ElementTypeId).ok_or_else(|| EngineError::not_found("the new type"))
}

/// The project's profile of a catalog row, imported when missing.
fn catalog_profile(
    s: &mut Session,
    (series, designation): (&str, &str),
    material: StructuralMaterial,
) -> Result<String> {
    let existing = s.project().profile_named(designation);
    if let Some(p) = existing.filter(|p| StructuralMaterial::of_profile(p.material) == material) {
        return Ok(p.name.clone());
    }
    // A same-named profile of another material is left alone: the row comes in under a free name.
    let out = ProfileImport {
        series: series.into(),
        designations: vec![designation.into()],
        on_clash: Some("rename".into()),
    }
    .run(s)?;
    out["imported"][0]["name"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| EngineError::not_found(format!("catalog row {designation}")))
}

/// The 0.20 x 0.40 concrete rectangle: the project's `20x40` when it is that, else a new one.
fn concrete_profile(s: &mut Session) -> Result<String> {
    let wanted = ProfileShape::Rect { b: 0.2, h: 0.4, chamfer: 0.0 };
    let existing = s.project().profile_named(CONCRETE_DEFAULT);
    if let Some(p) = existing.filter(|p| p.shape == wanted && p.material == ProfileMaterial::Concrete) {
        return Ok(p.name.clone());
    }
    let name = existing.is_none().then(|| CONCRETE_DEFAULT.to_string());
    let params = BTreeMap::from([("b".to_string(), 0.2), ("h".to_string(), 0.4)]);
    let out = ProfileCreate {
        name,
        material: "concrete".into(),
        shape: Some("rect".into()),
        params: Some(params),
        ..Default::default()
    }
    .run(s)?;
    out["name"].as_str().map(str::to_string).ok_or_else(|| EngineError::not_found("the new profile"))
}

/// The column or beam type new members of a structural material get: the
/// first type of that material, else a default made in one undo step
/// (steel `W200x26,6`, timber `MLC 9x32` from the profile catalog, concrete
/// a 0.20 x 0.40 rectangle named `20x40`). Returns its `id`, `name` and
/// whether it was `created`.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct StructuralTypeFor {
    /// `column` or `beam`.
    pub category: String,
    /// `concrete`, `steel` or `timber`.
    pub material: String,
}

impl Command for StructuralTypeFor {
    const NAME: &'static str = "structural_type_for";
    const KIND: CommandKind = CommandKind::Edit;
    fn run(self, s: &mut Session) -> Result<Value> {
        let category = member_category(&self.category)?;
        let material = material_arg(&self.material)?;
        let (id, created) = structural_type(s, category, material)?;
        let name = s.project().type_name(id).to_string();
        Ok(json!({ "id": id.0, "name": name, "created": created }))
    }
}
