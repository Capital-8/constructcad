//! Property and quantity sets carried over from IFC files: named sets of
//! named values, each value keeping its IFC measure type. Lengths, areas,
//! volumes and angles are stored in meters, square and cubic meters and
//! radians, whatever the units of the file they came from.

use serde::{Deserialize, Serialize};

/// A property set (`IfcPropertySet`) or quantity set (`IfcElementQuantity`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertySet {
    pub name: String,
    /// A quantity set rather than a property set.
    #[serde(default)]
    pub quantities: bool,
    #[serde(default)]
    pub properties: Vec<Property>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Property {
    pub name: String,
    pub value: PropertyValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PropertyValue {
    /// One value (or none).
    Single(Option<IfcValue>),
    /// Values chosen from an enumeration.
    Enumerated(Vec<IfcValue>),
    List(Vec<IfcValue>),
    Bounded {
        lower: Option<IfcValue>,
        upper: Option<IfcValue>,
        set_point: Option<IfcValue>,
    },
    Table {
        defining: Vec<IfcValue>,
        defined: Vec<IfcValue>,
    },
    /// Nested properties with a usage name.
    Complex {
        usage: String,
        properties: Vec<Property>,
    },
    /// A quantity: its entity (`IFCQUANTITYLENGTH`…) and value.
    Quantity {
        kind: String,
        value: f64,
    },
}

/// A value with its IFC defined type (`IFCLABEL`, `IFCLENGTHMEASURE`…).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IfcValue {
    pub measure: String,
    pub value: Atom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Atom {
    Text(String),
    Real(f64),
    Integer(i64),
    Bool(bool),
    /// Other enumeration literals (such as an unknown logical, `U`).
    Enum(String),
}

impl Atom {
    pub fn is_finite(&self) -> bool {
        !matches!(self, Atom::Real(x) if !x.is_finite())
    }
}

impl IfcValue {
    pub fn is_finite(&self) -> bool {
        self.value.is_finite()
    }
}

impl PropertyValue {
    /// Every number is finite (nested properties included).
    pub fn is_finite(&self) -> bool {
        let all = |v: &[IfcValue]| v.iter().all(IfcValue::is_finite);
        let opt = |v: &Option<IfcValue>| v.as_ref().is_none_or(IfcValue::is_finite);
        match self {
            PropertyValue::Single(v) => opt(v),
            PropertyValue::Enumerated(v) | PropertyValue::List(v) => all(v),
            PropertyValue::Bounded { lower, upper, set_point } => opt(lower) && opt(upper) && opt(set_point),
            PropertyValue::Table { defining, defined } => all(defining) && all(defined),
            PropertyValue::Complex { properties, .. } => properties.iter().all(|p| p.value.is_finite()),
            PropertyValue::Quantity { value, .. } => value.is_finite(),
        }
    }
}

/// Drop properties with non-finite numbers; returns how many went.
pub fn repair_sets(sets: &mut [PropertySet]) -> usize {
    let mut removed = 0;
    for set in sets {
        let before = set.properties.len();
        set.properties.retain(|p| p.value.is_finite());
        removed += before - set.properties.len();
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_drops_non_finite_values() {
        let real = |x: f64| IfcValue { measure: "IFCREAL".into(), value: Atom::Real(x) };
        let prop = |name: &str, value| Property { name: name.into(), value };
        let nested = PropertyValue::Complex {
            usage: String::new(),
            properties: vec![prop("inner", PropertyValue::List(vec![real(1.0), real(f64::INFINITY)]))],
        };
        let mut sets = vec![PropertySet {
            name: "S".into(),
            quantities: false,
            properties: vec![
                prop("ok", PropertyValue::Single(Some(real(1.0)))),
                prop("nan", PropertyValue::Single(Some(real(f64::NAN)))),
                prop("nested", nested),
                prop("q", PropertyValue::Quantity { kind: "IFCQUANTITYLENGTH".into(), value: f64::NAN }),
            ],
        }];
        assert_eq!(repair_sets(&mut sets), 3);
        assert_eq!(sets[0].properties.len(), 1);
    }
}
