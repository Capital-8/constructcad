//! The commands, by area. Each is a parameter struct implementing
//! [`crate::Command`]; [`crate::catalog`] lists them all.

pub mod create;
pub mod edit;
pub mod io;
pub mod levels;
pub mod materials;
pub mod modify;
pub mod project;
pub mod query;
pub mod types;
pub mod views;

use crate::error::{EngineError, Result};
use crate::session::Session;
use cc_core::{Category, EditCtx, Element, ElementId, ElementKind, ElementTypeId, LevelId, Project};
use serde_json::{Value, json};

/// Run an edit as one undo step, kept only if it succeeds and leaves a
/// project that needs no repair (no dangling references, no invalid numbers).
pub(crate) fn edit<R>(s: &mut Session, tx: String, f: impl FnOnce(&mut EditCtx) -> Result<R>) -> Result<R> {
    s.doc.try_edit(tx, f, check)
}

/// The project is consistent: repairing a copy would change nothing.
pub(crate) fn check(p: &Project) -> Result<()> {
    match p.clone().repair() {
        0 => Ok(()),
        n => Err(EngineError::rejected(format!("the edit would leave the model invalid ({n} problems)"))),
    }
}

/// Add an element (doors and windows get the next free mark).
pub(crate) fn add_element(e: &mut EditCtx, level: LevelId, type_id: ElementTypeId, kind: ElementKind) -> ElementId {
    let id = ElementId(e.alloc_id());
    let mut element =
        Element { id, level, type_id, mark: String::new(), comments: String::new(), kind, uid: Default::default() };
    if matches!(element.category(), Category::Door | Category::Window) {
        element.mark = e.project().next_mark(element.category());
    }
    e.add_element(element)
}

/// The fields of an enum value's single variant: `{"Wall": {...}}` gives
/// `("Wall", {...})`; unit variants give an empty object.
pub(crate) fn variant(v: Value) -> (String, Value) {
    match v {
        Value::Object(o) if o.len() == 1 => match o.into_iter().next() {
            Some((k, inner)) => (k, inner),
            None => (String::new(), json!({})),
        },
        Value::String(name) => (name, json!({})),
        other => (String::new(), other),
    }
}

/// JSON merge patch (RFC 7396): objects merge key by key, `null` removes,
/// anything else replaces.
pub(crate) fn merge(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(t), Value::Object(p)) => {
            for (k, v) in p {
                if v.is_null() {
                    t.remove(k);
                } else {
                    merge(t.entry(k.clone()).or_insert(Value::Null), v);
                }
            }
        }
        (t, p) => *t = p.clone(),
    }
}

/// Apply a merge patch to the fields of an enum value (keeping its variant).
pub(crate) fn patch_variant<T: serde::Serialize + serde::de::DeserializeOwned>(value: &T, patch: &Value) -> Result<T> {
    let (name, mut fields) = variant(serde_json::to_value(value).map_err(|e| EngineError::invalid(e.to_string()))?);
    merge(&mut fields, patch);
    let rebuilt = if fields.as_object().is_some_and(|o| o.is_empty()) && !patch.is_object() {
        Value::String(name)
    } else {
        json!({ name: fields })
    };
    serde_json::from_value(rebuilt).map_err(|e| EngineError::invalid(format!("invalid params: {e}")))
}

/// Apply a merge patch to a whole serializable value.
pub(crate) fn patch_value<T: serde::Serialize + serde::de::DeserializeOwned>(value: &T, patch: &Value) -> Result<T> {
    let mut v = serde_json::to_value(value).map_err(|e| EngineError::invalid(e.to_string()))?;
    merge(&mut v, patch);
    serde_json::from_value(v).map_err(|e| EngineError::invalid(format!("invalid values: {e}")))
}

/// What a created or changed element looks like now.
pub(crate) fn created(s: &mut Session, id: ElementId) -> Value {
    query::element_json(s, id).unwrap_or_else(|| json!({ "id": id.0 }))
}

pub(crate) fn ids_json(ids: &[ElementId]) -> Value {
    json!(ids.iter().map(|i| i.0).collect::<Vec<_>>())
}
