//! Material libraries: the materials of one region or standard, as a file
//! (JSON, format [`FILE_FORMAT`]) that the user picks and imports, as the
//! pipe series of [`crate::pipe_catalog`] are. The built-in libraries are
//! files in `assets/materials`, compiled into the binary and read once;
//! users write their own with [`write_library`] (and the engine's
//! `material_library_export`). Importing copies materials into
//! [`Project::materials`](crate::Project): a project keeps working when a
//! library changes.

use crate::material::{Finish, Material, MaterialClass, template_properties};
use crate::pipe_catalog::Region;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

/// The file format this module reads and writes.
pub const FILE_FORMAT: u64 = 1;
/// The largest library file read, bytes.
pub const MAX_FILE_BYTES: usize = 4 * 1024 * 1024;
/// The most materials in one library.
pub const MAX_MATERIALS: usize = 2000;
/// The longest code, name, standard or material name, characters.
const MAX_TEXT: usize = 200;
/// The longest description or source, characters.
const MAX_LONG_TEXT: usize = 2000;
/// The most sources listed.
const MAX_SOURCES: usize = 50;
/// The most translated names of one material.
const MAX_NAMES: usize = 32;

/// A library: its code (unique among libraries), name, region and standard,
/// the documents its values come from, and its materials.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialLibrary {
    pub format: u64,
    pub code: String,
    pub name: String,
    /// A [`Region`] code (`brazil`, `europe`, `us`), or empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub region: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub standard: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    pub materials: Vec<LibraryMaterial>,
}

impl MaterialLibrary {
    /// The library's region, when its code is one of [`Region`]'s.
    pub fn region(&self) -> Option<Region> {
        Region::ALL.into_iter().find(|r| r.code() == self.region)
    }

    /// The material named `name` (case aside): its main name first, then
    /// any of its translated names.
    pub fn material(&self, name: &str) -> Option<&LibraryMaterial> {
        let name = name.trim();
        self.materials
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(name))
            .or_else(|| self.materials.iter().find(|m| m.names.values().any(|n| n.eq_ignore_ascii_case(name))))
    }
}

/// One material of a library: SI units as in [`crate::material::MaterialPhysical`]
/// (kg/m³, W/(m·K), J/(kg·K), 1/K, MPa). Values left out take the class's
/// template ([`template_properties`]) or stay unknown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryMaterial {
    /// The main name (the library's own language).
    pub name: String,
    /// The name in other languages, by language code (`en`, `es`, `zh-CN`…).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub names: BTreeMap<String, String>,
    #[serde(default)]
    pub class: MaterialClass,
    #[serde(default = "default_color")]
    pub color: [u8; 3],
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The appearance's finish (matte when left out): a gloss paint, a polished stone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish: Option<Finish>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thermal_conductivity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specific_heat: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thermal_expansion: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub youngs_modulus: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poisson: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compressive_strength: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yield_strength: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tensile_strength: Option<f64>,
}

fn default_color() -> [u8; 3] {
    [180, 180, 180]
}

impl LibraryMaterial {
    /// The name in language `lang` (a code such as `en`), else the main name.
    pub fn name_in(&self, lang: &str) -> &str {
        self.names.get(lang).map_or(self.name.as_str(), String::as_str)
    }

    /// Every name it goes by: the main one and the translated ones.
    pub fn all_names(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.name.as_str()).chain(self.names.values().map(String::as_str))
    }

    /// The project material it makes: the class's template, then the library's values.
    pub fn to_material(&self) -> Material {
        self.to_material_in("")
    }

    /// [`Self::to_material`], named in language `lang` ([`Self::name_in`]).
    pub fn to_material_in(&self, lang: &str) -> Material {
        let mut m = Material::new(self.name_in(lang).trim(), self.color);
        template_properties(&mut m, self.class);
        m.identity.class = self.class;
        m.identity.description = self.description.clone();
        if let Some(finish) = self.finish {
            m.appearance.finish = finish;
        }
        let p = &mut m.physical;
        for (value, field) in [
            (self.density, &mut p.density),
            (self.thermal_conductivity, &mut p.thermal_conductivity),
            (self.specific_heat, &mut p.specific_heat),
            (self.thermal_expansion, &mut p.thermal_expansion),
            (self.youngs_modulus, &mut p.youngs_modulus),
            (self.poisson, &mut p.poisson),
            (self.compressive_strength, &mut p.compressive_strength),
            (self.yield_strength, &mut p.yield_strength),
            (self.tensile_strength, &mut p.tensile_strength),
        ] {
            if value.is_some() {
                *field = value;
            }
        }
        m
    }

    /// A project material as a library row (its identity, color and physical values).
    pub fn of_material(m: &Material) -> Self {
        let p = &m.physical;
        Self {
            name: m.name.clone(),
            names: BTreeMap::new(),
            class: m.identity.class,
            color: m.color,
            description: m.identity.description.clone(),
            finish: (m.appearance.finish != Finish::Matte).then_some(m.appearance.finish),
            density: p.density,
            thermal_conductivity: p.thermal_conductivity,
            specific_heat: p.specific_heat,
            thermal_expansion: p.thermal_expansion,
            youngs_modulus: p.youngs_modulus,
            poisson: p.poisson,
            compressive_strength: p.compressive_strength,
            yield_strength: p.yield_strength,
            tensile_strength: p.tensile_strength,
        }
    }

    /// The physical values with their names, for checks.
    fn values(&self) -> [(&'static str, Option<f64>); 9] {
        [
            ("density", self.density),
            ("thermal_conductivity", self.thermal_conductivity),
            ("specific_heat", self.specific_heat),
            ("thermal_expansion", self.thermal_expansion),
            ("youngs_modulus", self.youngs_modulus),
            ("poisson", self.poisson),
            ("compressive_strength", self.compressive_strength),
            ("yield_strength", self.yield_strength),
            ("tensile_strength", self.tensile_strength),
        ]
    }
}

/// Why a library file cannot be read.
#[derive(Debug, Clone, PartialEq)]
pub enum LibraryError {
    TooLarge(usize),
    Json(String),
    /// A format this version does not read.
    Format(u64),
    /// The library's fields.
    Header(String),
    /// A material row (counted from 0).
    Material {
        row: usize,
        message: String,
    },
}

impl std::fmt::Display for LibraryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge(n) => write!(f, "the file has {n} bytes, more than {MAX_FILE_BYTES}"),
            Self::Json(e) => write!(f, "not a material library: {e}"),
            Self::Format(v) => write!(f, "library format {v} is not supported (expected {FILE_FORMAT})"),
            Self::Header(e) => write!(f, "{e}"),
            Self::Material { row, message } => write!(f, "material {}: {message}", row + 1),
        }
    }
}

/// Read a library file, bounded and checked: a code and a name, at most
/// [`MAX_MATERIALS`] materials with distinct non-empty names, finite values
/// that are not negative (Poisson's ratio from 0 to 0.5).
pub fn read_library(text: &str) -> Result<MaterialLibrary, LibraryError> {
    if text.len() > MAX_FILE_BYTES {
        return Err(LibraryError::TooLarge(text.len()));
    }
    let value: Value = serde_json::from_str(text).map_err(|e| LibraryError::Json(e.to_string()))?;
    let Some(format) = value.get("format") else {
        return Err(LibraryError::Json("no format".into()));
    };
    match format.as_u64() {
        Some(FILE_FORMAT) => {}
        Some(other) => return Err(LibraryError::Format(other)),
        None => return Err(LibraryError::Header(format!("format {format} is not a number"))),
    }
    let rows = match value.get("materials") {
        Some(Value::Array(rows)) => rows.len(),
        _ => return Err(LibraryError::Header("no materials list".into())),
    };
    if rows > MAX_MATERIALS {
        return Err(LibraryError::Header(format!("more than {MAX_MATERIALS} materials")));
    }
    let mut lib: MaterialLibrary = serde_json::from_value(value).map_err(|e| LibraryError::Header(e.to_string()))?;
    lib.code = lib.code.trim().to_string();
    lib.name = lib.name.trim().to_string();
    if lib.code.is_empty() || lib.name.is_empty() {
        return Err(LibraryError::Header("a library needs a code and a name".into()));
    }
    let long = [&lib.code, &lib.name, &lib.standard, &lib.region].into_iter().any(|t| t.chars().count() > MAX_TEXT);
    if long || lib.sources.len() > MAX_SOURCES || lib.sources.iter().any(|s| s.chars().count() > MAX_LONG_TEXT) {
        return Err(LibraryError::Header("a field is too long".into()));
    }
    let mut names = BTreeSet::new();
    for (row, m) in lib.materials.iter_mut().enumerate() {
        let fail = |message: String| LibraryError::Material { row, message };
        m.name = m.name.trim().to_string();
        if m.name.is_empty() || m.name.chars().count() > MAX_TEXT {
            return Err(fail("a name of 1 to 200 characters is needed".into()));
        }
        if m.description.chars().count() > MAX_LONG_TEXT {
            return Err(fail("the description is too long".into()));
        }
        if !names.insert(m.name.to_lowercase()) {
            return Err(fail(format!("\"{}\" appears twice", m.name)));
        }
        if m.names.len() > MAX_NAMES {
            return Err(fail(format!("more than {MAX_NAMES} translated names")));
        }
        let names_in = std::mem::take(&mut m.names);
        for (lang, name) in names_in {
            let ok_code = (1..=16).contains(&lang.len()) && lang.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
            let name = name.trim().to_string();
            if !ok_code || name.is_empty() || name.chars().count() > MAX_TEXT {
                return Err(fail(format!("the name in \"{lang}\" is not valid")));
            }
            m.names.insert(lang, name);
        }
        for (field, v) in m.values() {
            let Some(v) = v else { continue };
            let poisson = field == "poisson";
            if !v.is_finite() || v < 0.0 || (poisson && v > 0.5) {
                return Err(fail(format!("{field} = {v} is out of range")));
            }
        }
    }
    // In each language the names are distinct too.
    let langs: BTreeSet<&String> = lib.materials.iter().flat_map(|m| m.names.keys()).collect();
    for lang in langs {
        let mut seen = BTreeSet::new();
        for (row, m) in lib.materials.iter().enumerate() {
            if !seen.insert(m.name_in(lang).to_lowercase()) {
                let message = format!("\"{}\" appears twice in \"{lang}\"", m.name_in(lang));
                return Err(LibraryError::Material { row, message });
            }
        }
    }
    Ok(lib)
}

/// The library as a file, readable by [`read_library`].
pub fn write_library(lib: &MaterialLibrary) -> String {
    let lib = MaterialLibrary { format: FILE_FORMAT, ..lib.clone() };
    serde_json::to_string_pretty(&lib).unwrap_or_default() + "\n"
}

/// A built-in library file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltIn {
    /// The library's code, as in the file.
    pub code: &'static str,
    /// The i18n key of the library's name: `material-library-<code lowercased>`.
    pub name_key: &'static str,
    /// The region whose standards it follows; `None` for an international
    /// library (materials by their physical kind: paints, stones, glass).
    pub region: Option<Region>,
    /// The file's name in `assets/materials`.
    pub file: &'static str,
    /// The file's text.
    pub text: &'static str,
}

/// Every built-in library file.
pub const BUILT_IN: &[BuiltIn] = &[
    BuiltIn {
        code: "BR-ACO-ESTRUTURAL",
        name_key: "material-library-br-aco-estrutural",
        region: Some(Region::Brazil),
        file: "br-aco-estrutural.json",
        text: include_str!("../assets/materials/br-aco-estrutural.json"),
    },
    BuiltIn {
        code: "BR-ACO-ARMADURA",
        name_key: "material-library-br-aco-armadura",
        region: Some(Region::Brazil),
        file: "br-aco-armadura.json",
        text: include_str!("../assets/materials/br-aco-armadura.json"),
    },
    BuiltIn {
        code: "BR-CONCRETO",
        name_key: "material-library-br-concreto",
        region: Some(Region::Brazil),
        file: "br-concreto.json",
        text: include_str!("../assets/materials/br-concreto.json"),
    },
    BuiltIn {
        code: "PAINTS",
        name_key: "material-library-paints",
        region: None,
        file: "paints.json",
        text: include_str!("../assets/materials/paints.json"),
    },
    BuiltIn {
        code: "CERAMICS",
        name_key: "material-library-ceramics",
        region: None,
        file: "ceramics.json",
        text: include_str!("../assets/materials/ceramics.json"),
    },
    BuiltIn {
        code: "STONES",
        name_key: "material-library-stones",
        region: None,
        file: "stones.json",
        text: include_str!("../assets/materials/stones.json"),
    },
    BuiltIn {
        code: "MORTARS-PLASTERS",
        name_key: "material-library-mortars-plasters",
        region: None,
        file: "mortars-plasters.json",
        text: include_str!("../assets/materials/mortars-plasters.json"),
    },
    BuiltIn {
        code: "WOODS",
        name_key: "material-library-woods",
        region: None,
        file: "woods.json",
        text: include_str!("../assets/materials/woods.json"),
    },
    BuiltIn {
        code: "GLASS",
        name_key: "material-library-glass",
        region: None,
        file: "glass.json",
        text: include_str!("../assets/materials/glass.json"),
    },
    BuiltIn {
        code: "BR-CERAMICAS",
        name_key: "material-library-br-ceramicas",
        region: Some(Region::Brazil),
        file: "br-ceramicas.json",
        text: include_str!("../assets/materials/br-ceramicas.json"),
    },
    BuiltIn {
        code: "BR-PEDRAS",
        name_key: "material-library-br-pedras",
        region: Some(Region::Brazil),
        file: "br-pedras.json",
        text: include_str!("../assets/materials/br-pedras.json"),
    },
    BuiltIn {
        code: "BR-ARGAMASSAS-GESSO",
        name_key: "material-library-br-argamassas-gesso",
        region: Some(Region::Brazil),
        file: "br-argamassas-gesso.json",
        text: include_str!("../assets/materials/br-argamassas-gesso.json"),
    },
    BuiltIn {
        code: "BR-MADEIRAS",
        name_key: "material-library-br-madeiras",
        region: Some(Region::Brazil),
        file: "br-madeiras.json",
        text: include_str!("../assets/materials/br-madeiras.json"),
    },
    BuiltIn {
        code: "BR-ISOLANTES",
        name_key: "material-library-br-isolantes",
        region: Some(Region::Brazil),
        file: "br-isolantes.json",
        text: include_str!("../assets/materials/br-isolantes.json"),
    },
];

/// The built-in libraries, read once, in [`BUILT_IN`]'s order (a file
/// that does not read is left out; a test reads them all).
pub fn built_in() -> &'static [(BuiltIn, MaterialLibrary)] {
    static LIBS: OnceLock<Vec<(BuiltIn, MaterialLibrary)>> = OnceLock::new();
    LIBS.get_or_init(|| BUILT_IN.iter().filter_map(|b| read_library(b.text).ok().map(|lib| (*b, lib))).collect())
}

/// The built-in library of `code` (case aside).
pub fn find(code: &str) -> Option<&'static (BuiltIn, MaterialLibrary)> {
    built_in().iter().find(|(b, _)| b.code.eq_ignore_ascii_case(code.trim()))
}

/// NBR 6118:2023, 8.2.8: the initial tangent modulus `Eci` and the secant
/// modulus `Ecs` (MPa) of a concrete of characteristic strength `fck_mpa`,
/// with `αE = 1.0` (granite and gneiss). The built-in concretes' moduli are
/// `Ecs` rounded to 100 MPa.
pub fn concrete_modulus_nbr6118(fck_mpa: f64) -> (f64, f64) {
    let fck = fck_mpa.max(0.0);
    let eci = if fck <= 50.0 { 5600.0 * fck.sqrt() } else { 21_500.0 * (fck / 10.0 + 1.25).cbrt() };
    let alpha_i = (0.8 + 0.2 * fck / 80.0).min(1.0);
    (eci, alpha_i * eci)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_built_in_library_reads_and_matches_its_entry() {
        assert_eq!(built_in().len(), BUILT_IN.len(), "every file reads");
        let mut codes = std::collections::BTreeSet::new();
        for (b, lib) in built_in() {
            assert_eq!(lib.code, b.code, "{}", b.file);
            assert_eq!(lib.region(), b.region, "{}", b.file);
            assert_eq!(b.name_key, format!("material-library-{}", b.code.to_lowercase()));
            assert!(codes.insert(b.code), "{} repeats", b.code);
            assert!(!lib.materials.is_empty() && !lib.sources.is_empty(), "{}", b.file);
        }
    }

    /// The values the Brazilian libraries hold, against the standards they cite.
    #[test]
    fn brazilian_values_follow_their_standards() {
        let steel = &find("br-aco-estrutural").unwrap().1;
        let mr250 = steel.material("Aço MR250 (NBR 7007)").unwrap().to_material();
        assert_eq!(mr250.identity.class, MaterialClass::Metal);
        assert_eq!(mr250.physical.density, Some(7850.0));
        assert_eq!(mr250.physical.youngs_modulus, Some(200_000.0));
        assert_eq!((mr250.physical.yield_strength, mr250.physical.tensile_strength), (Some(250.0), Some(400.0)));
        // Yield is no longer kept as the compressive strength.
        assert_eq!(mr250.physical.compressive_strength, None);
        let rebar = &find("BR-ACO-ARMADURA").unwrap().1;
        let ca50 = rebar.material("Aço CA-50 (NBR 7480)").unwrap();
        assert_eq!((ca50.yield_strength, ca50.youngs_modulus), (Some(500.0), Some(210_000.0)));
        let concrete = &find("BR-CONCRETO").unwrap().1;
        assert_eq!(concrete.materials.len(), 12, "C20 to C90");
        for m in &concrete.materials {
            let fck = m.compressive_strength.unwrap();
            let ecs = (concrete_modulus_nbr6118(fck).1 / 100.0).round() * 100.0;
            assert_eq!(m.youngs_modulus, Some(ecs), "{}", m.name);
        }
        // NBR 6118 Table 8.1: C30 has Ecs = 27 GPa, C90 47 GPa (rounded there).
        let c30 = concrete.material("Concreto C30 (NBR 6118)").unwrap().youngs_modulus.unwrap();
        let c90 = concrete.material("Concreto C90 (NBR 6118)").unwrap().youngs_modulus.unwrap();
        assert!((c30 / 1000.0 - 27.0).abs() < 0.5 && (c90 / 1000.0 - 47.0).abs() < 0.5, "{c30} {c90}");
    }

    /// The finish libraries: the international ones hold ISO 10456 values
    /// only, the Brazilian ones the NBR 15220-2 bands; classes and gloss.
    #[test]
    fn finish_libraries_keep_their_classes_finishes_and_sources() {
        let lib = |code: &str| &find(code).unwrap().1;
        for m in &lib("PAINTS").materials {
            assert_eq!(m.class, MaterialClass::Paint, "{}", m.name);
            assert!(m.values().iter().all(|(_, v)| v.is_none()), "a film has no physical values: {}", m.name);
        }
        let epoxy = lib("PAINTS").material("Tinta epóxi para piso cinza").unwrap().to_material();
        assert_eq!(epoxy.appearance.finish, Finish::Glossy);
        let porcelain = lib("CERAMICS").material("Porcelanato polido").unwrap().to_material();
        assert_eq!((porcelain.identity.class, porcelain.appearance.finish), (MaterialClass::Ceramic, Finish::Polished));
        assert_eq!(porcelain.physical.thermal_conductivity, Some(1.3));
        // International libraries: no region, no Brazilian standard in their values.
        for (b, l) in built_in().iter().filter(|(b, _)| b.region.is_none()) {
            assert!(!b.code.starts_with("BR-"), "{}", b.code);
            assert!(l.sources.iter().all(|s| !s.contains("15220")), "{}", b.code);
            assert!(l.materials.iter().all(|m| !m.description.contains("NBR")), "{}", b.code);
        }
        // ISO 10456: stones c = 1 000, granite λ = 2.8, soda-lime glass c = 750.
        for m in &lib("STONES").materials {
            assert_eq!((m.class, m.specific_heat), (MaterialClass::Stone, Some(1000.0)), "{}", m.name);
        }
        assert_eq!(lib("STONES").material("Granito preto polido").unwrap().thermal_conductivity, Some(2.8));
        let glass = lib("GLASS").material("Vidro float incolor").unwrap().to_material();
        assert_eq!((glass.physical.youngs_modulus, glass.physical.specific_heat), (Some(70_000.0), Some(750.0)));
        // NBR 15220-2, Table B.3: the wood conductivity follows the density band.
        let wood_band = |rho: f64| match rho {
            r if r >= 800.0 => 0.29,
            r if r >= 600.0 => 0.23,
            r if r >= 450.0 => 0.15,
            _ => 0.12,
        };
        for m in &lib("BR-MADEIRAS").materials {
            assert_eq!(m.thermal_conductivity, Some(wood_band(m.density.unwrap())), "{}", m.name);
        }
        // Ceramics of 1 300 to 2 000 kg/m³ as in the same table.
        for m in &lib("BR-CERAMICAS").materials {
            let rho = m.density.unwrap();
            let lambda = if rho > 1800.0 {
                1.05
            } else if rho > 1600.0 {
                1.00
            } else {
                0.90
            };
            assert_eq!((m.thermal_conductivity, m.specific_heat), (Some(lambda), Some(920.0)), "{}", m.name);
        }
        let eps = lib("BR-ISOLANTES").material("Poliestireno expandido (EPS)").unwrap();
        assert_eq!((eps.density, eps.thermal_conductivity), (Some(15.0), Some(0.040)));
        // A finish survives a project material and back.
        let row = LibraryMaterial::of_material(&porcelain);
        assert_eq!(row.finish, Some(Finish::Polished));
        assert_eq!(LibraryMaterial::of_material(&Material::new("Plain", [1, 2, 3])).finish, None, "matte is left out");
    }

    /// A material goes by its main name and its translated ones; it takes
    /// the one of the language asked for.
    #[test]
    fn translated_names_are_found_and_used() {
        let text = r#"{"format": 1, "code": "X", "name": "X", "materials": [
            {"name": "Vidro", "names": {"en": " Glass ", "es": "Vidrio"}},
            {"name": "Pedra", "names": {"en": "Stone"}}]}"#;
        let lib = read_library(text).unwrap();
        let glass = lib.material("glass").unwrap();
        assert_eq!((glass.name.as_str(), glass.name_in("en"), glass.name_in("fr")), ("Vidro", "Glass", "Vidro"));
        assert_eq!(glass.to_material_in("es").name, "Vidrio");
        assert_eq!(lib.material("Pedra").unwrap().to_material().name, "Pedra");
        assert_eq!(glass.all_names().count(), 3);
        // Two materials of the same name in one language, a bad code, an empty name.
        for bad in [
            r#"[{"name": "A", "names": {"en": "C"}}, {"name": "B", "names": {"en": "c"}}]"#,
            r#"[{"name": "A", "names": {"en": "B"}}, {"name": "B"}]"#,
            r#"[{"name": "A", "names": {"e n": "B"}}]"#,
            r#"[{"name": "A", "names": {"en": "  "}}]"#,
        ] {
            let text = format!(r#"{{"format": 1, "code": "X", "name": "X", "materials": {bad}}}"#);
            assert!(matches!(read_library(&text), Err(LibraryError::Material { .. })), "{bad}");
        }
    }

    #[test]
    fn files_round_trip_and_hostile_ones_are_refused() {
        for (_, lib) in built_in() {
            assert_eq!(read_library(&write_library(lib)).as_ref(), Ok(lib), "{}", lib.code);
        }
        let base = |materials: &str| format!(r#"{{"format": 1, "code": "X", "name": "X", "materials": {materials}}}"#);
        assert!(read_library(&base(r#"[{"name": "A"}]"#)).is_ok());
        assert!(matches!(
            read_library(&base(r#"[{"name": "A"}, {"name": " a "}]"#)),
            Err(LibraryError::Material { row: 1, .. })
        ));
        assert!(matches!(
            read_library(&base(r#"[{"name": "A", "poisson": 0.7}]"#)),
            Err(LibraryError::Material { .. })
        ));
        assert!(matches!(read_library(&base(r#"[{"name": "A", "density": -1}]"#)), Err(LibraryError::Material { .. })));
        assert!(matches!(read_library(&base(r#"[{"name": ""}]"#)), Err(LibraryError::Material { .. })));
        assert!(matches!(read_library(r#"{"format": 2, "materials": []}"#), Err(LibraryError::Format(2))));
        assert!(matches!(
            read_library(r#"{"format": 1, "code": "", "name": "X", "materials": []}"#),
            Err(LibraryError::Header(_))
        ));
        assert!(matches!(read_library("not json"), Err(LibraryError::Json(_))));
        let many = format!("[{}]", vec![r#"{"name": "A"}"#; MAX_MATERIALS + 1].join(","));
        assert!(matches!(read_library(&base(&many)), Err(LibraryError::Header(_))));
    }
}
