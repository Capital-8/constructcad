//! The part of the IFC4 schema needed to write any physical element class
//! back out: the attribute count of each `IfcElement` subtype, where its
//! `PredefinedType` sits and which type object goes with it. Taken from the
//! published IFC4 ADD2 TC1 schema; all the attributes after the common eight
//! are optional, so the writer fills them with `$`.

/// Extra attributes of a type object after the nine of `IfcElementType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeLayout {
    /// `PredefinedType`.
    Predefined,
    /// No further attributes.
    Plain,
    /// `AssemblyPlace`, `PredefinedType` (furniture).
    Furniture,
    /// `PredefinedType`, `OperationType`, `ParameterTakesPrecedence`, `UserDefinedOperationType`.
    Door,
    /// `PredefinedType`, `PartitioningType`, `ParameterTakesPrecedence`, `UserDefinedPartitioningType`.
    Window,
}

/// How an element class is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElementClass {
    pub entity: &'static str,
    /// Number of attributes of an occurrence.
    pub attrs: usize,
    /// Index of `PredefinedType` in an occurrence.
    pub predefined: Option<usize>,
    /// Type object entity (none when the schema only has an abstract one).
    pub type_entity: Option<(&'static str, TypeLayout)>,
}

const fn class(entity: &'static str, type_entity: &'static str) -> ElementClass {
    ElementClass { entity, attrs: 9, predefined: Some(8), type_entity: Some((type_entity, TypeLayout::Predefined)) }
}

const fn untyped(entity: &'static str) -> ElementClass {
    ElementClass { entity, attrs: 8, predefined: None, type_entity: None }
}

const CLASSES: &[ElementClass] = &[
    // Building elements.
    class("IFCBEAM", "IFCBEAMTYPE"),
    class("IFCBEAMSTANDARDCASE", "IFCBEAMTYPE"),
    class("IFCBUILDINGELEMENTPROXY", "IFCBUILDINGELEMENTPROXYTYPE"),
    class("IFCCHIMNEY", "IFCCHIMNEYTYPE"),
    class("IFCCOLUMN", "IFCCOLUMNTYPE"),
    class("IFCCOLUMNSTANDARDCASE", "IFCCOLUMNTYPE"),
    class("IFCCOVERING", "IFCCOVERINGTYPE"),
    class("IFCCURTAINWALL", "IFCCURTAINWALLTYPE"),
    ElementClass {
        entity: "IFCDOOR",
        attrs: 13,
        predefined: Some(10),
        type_entity: Some(("IFCDOORTYPE", TypeLayout::Door)),
    },
    ElementClass {
        entity: "IFCDOORSTANDARDCASE",
        attrs: 13,
        predefined: Some(10),
        type_entity: Some(("IFCDOORTYPE", TypeLayout::Door)),
    },
    class("IFCFOOTING", "IFCFOOTINGTYPE"),
    class("IFCMEMBER", "IFCMEMBERTYPE"),
    class("IFCMEMBERSTANDARDCASE", "IFCMEMBERTYPE"),
    ElementClass {
        entity: "IFCPILE",
        attrs: 10,
        predefined: Some(8),
        type_entity: Some(("IFCPILETYPE", TypeLayout::Predefined)),
    },
    class("IFCPLATE", "IFCPLATETYPE"),
    class("IFCPLATESTANDARDCASE", "IFCPLATETYPE"),
    class("IFCRAILING", "IFCRAILINGTYPE"),
    class("IFCRAMP", "IFCRAMPTYPE"),
    class("IFCRAMPFLIGHT", "IFCRAMPFLIGHTTYPE"),
    class("IFCROOF", "IFCROOFTYPE"),
    class("IFCSHADINGDEVICE", "IFCSHADINGDEVICETYPE"),
    class("IFCSLAB", "IFCSLABTYPE"),
    class("IFCSLABELEMENTEDCASE", "IFCSLABTYPE"),
    class("IFCSLABSTANDARDCASE", "IFCSLABTYPE"),
    class("IFCSTAIR", "IFCSTAIRTYPE"),
    ElementClass {
        entity: "IFCSTAIRFLIGHT",
        attrs: 13,
        predefined: Some(12),
        type_entity: Some(("IFCSTAIRFLIGHTTYPE", TypeLayout::Predefined)),
    },
    class("IFCWALL", "IFCWALLTYPE"),
    class("IFCWALLELEMENTEDCASE", "IFCWALLTYPE"),
    class("IFCWALLSTANDARDCASE", "IFCWALLTYPE"),
    ElementClass {
        entity: "IFCWINDOW",
        attrs: 13,
        predefined: Some(10),
        type_entity: Some(("IFCWINDOWTYPE", TypeLayout::Window)),
    },
    ElementClass {
        entity: "IFCWINDOWSTANDARDCASE",
        attrs: 13,
        predefined: Some(10),
        type_entity: Some(("IFCWINDOWTYPE", TypeLayout::Window)),
    },
    // Other elements.
    ElementClass {
        entity: "IFCCIVILELEMENT",
        attrs: 8,
        predefined: None,
        type_entity: Some(("IFCCIVILELEMENTTYPE", TypeLayout::Plain)),
    },
    ElementClass {
        entity: "IFCELEMENTASSEMBLY",
        attrs: 10,
        predefined: Some(9),
        type_entity: Some(("IFCELEMENTASSEMBLYTYPE", TypeLayout::Predefined)),
    },
    ElementClass {
        entity: "IFCFURNISHINGELEMENT",
        attrs: 8,
        predefined: None,
        type_entity: Some(("IFCFURNISHINGELEMENTTYPE", TypeLayout::Plain)),
    },
    ElementClass {
        entity: "IFCFURNITURE",
        attrs: 9,
        predefined: Some(8),
        type_entity: Some(("IFCFURNITURETYPE", TypeLayout::Furniture)),
    },
    ElementClass {
        entity: "IFCSYSTEMFURNITUREELEMENT",
        attrs: 9,
        predefined: Some(8),
        type_entity: Some(("IFCSYSTEMFURNITUREELEMENTTYPE", TypeLayout::Predefined)),
    },
    class("IFCGEOGRAPHICELEMENT", "IFCGEOGRAPHICELEMENTTYPE"),
    class("IFCTRANSPORTELEMENT", "IFCTRANSPORTELEMENTTYPE"),
    class("IFCBUILDINGELEMENTPART", "IFCBUILDINGELEMENTPARTTYPE"),
    class("IFCDISCRETEACCESSORY", "IFCDISCRETEACCESSORYTYPE"),
    class("IFCFASTENER", "IFCFASTENERTYPE"),
    ElementClass {
        entity: "IFCMECHANICALFASTENER",
        attrs: 11,
        predefined: Some(10),
        type_entity: Some(("IFCMECHANICALFASTENERTYPE", TypeLayout::Predefined)),
    },
    class("IFCVIBRATIONISOLATOR", "IFCVIBRATIONISOLATORTYPE"),
    // Distribution elements (generic supertypes have only abstract types).
    ElementClass {
        entity: "IFCDISTRIBUTIONELEMENT",
        attrs: 8,
        predefined: None,
        type_entity: Some(("IFCDISTRIBUTIONELEMENTTYPE", TypeLayout::Plain)),
    },
    untyped("IFCDISTRIBUTIONCONTROLELEMENT"),
    untyped("IFCDISTRIBUTIONFLOWELEMENT"),
    untyped("IFCENERGYCONVERSIONDEVICE"),
    untyped("IFCFLOWCONTROLLER"),
    untyped("IFCFLOWFITTING"),
    untyped("IFCFLOWMOVINGDEVICE"),
    untyped("IFCFLOWSEGMENT"),
    untyped("IFCFLOWSTORAGEDEVICE"),
    untyped("IFCFLOWTERMINAL"),
    untyped("IFCFLOWTREATMENTDEVICE"),
    class("IFCDISTRIBUTIONCHAMBERELEMENT", "IFCDISTRIBUTIONCHAMBERELEMENTTYPE"),
    class("IFCACTUATOR", "IFCACTUATORTYPE"),
    class("IFCALARM", "IFCALARMTYPE"),
    class("IFCCONTROLLER", "IFCCONTROLLERTYPE"),
    class("IFCFLOWINSTRUMENT", "IFCFLOWINSTRUMENTTYPE"),
    class("IFCPROTECTIVEDEVICETRIPPINGUNIT", "IFCPROTECTIVEDEVICETRIPPINGUNITTYPE"),
    class("IFCSENSOR", "IFCSENSORTYPE"),
    class("IFCUNITARYCONTROLELEMENT", "IFCUNITARYCONTROLELEMENTTYPE"),
    class("IFCAIRTOAIRHEATRECOVERY", "IFCAIRTOAIRHEATRECOVERYTYPE"),
    class("IFCBOILER", "IFCBOILERTYPE"),
    class("IFCBURNER", "IFCBURNERTYPE"),
    class("IFCCHILLER", "IFCCHILLERTYPE"),
    class("IFCCOIL", "IFCCOILTYPE"),
    class("IFCCONDENSER", "IFCCONDENSERTYPE"),
    class("IFCCOOLEDBEAM", "IFCCOOLEDBEAMTYPE"),
    class("IFCCOOLINGTOWER", "IFCCOOLINGTOWERTYPE"),
    class("IFCELECTRICGENERATOR", "IFCELECTRICGENERATORTYPE"),
    class("IFCELECTRICMOTOR", "IFCELECTRICMOTORTYPE"),
    class("IFCENGINE", "IFCENGINETYPE"),
    class("IFCEVAPORATIVECOOLER", "IFCEVAPORATIVECOOLERTYPE"),
    class("IFCEVAPORATOR", "IFCEVAPORATORTYPE"),
    class("IFCHEATEXCHANGER", "IFCHEATEXCHANGERTYPE"),
    class("IFCHUMIDIFIER", "IFCHUMIDIFIERTYPE"),
    class("IFCMOTORCONNECTION", "IFCMOTORCONNECTIONTYPE"),
    class("IFCSOLARDEVICE", "IFCSOLARDEVICETYPE"),
    class("IFCTRANSFORMER", "IFCTRANSFORMERTYPE"),
    class("IFCTUBEBUNDLE", "IFCTUBEBUNDLETYPE"),
    class("IFCUNITARYEQUIPMENT", "IFCUNITARYEQUIPMENTTYPE"),
    class("IFCAIRTERMINALBOX", "IFCAIRTERMINALBOXTYPE"),
    class("IFCDAMPER", "IFCDAMPERTYPE"),
    class("IFCELECTRICDISTRIBUTIONBOARD", "IFCELECTRICDISTRIBUTIONBOARDTYPE"),
    class("IFCELECTRICTIMECONTROL", "IFCELECTRICTIMECONTROLTYPE"),
    class("IFCFLOWMETER", "IFCFLOWMETERTYPE"),
    class("IFCPROTECTIVEDEVICE", "IFCPROTECTIVEDEVICETYPE"),
    class("IFCSWITCHINGDEVICE", "IFCSWITCHINGDEVICETYPE"),
    class("IFCVALVE", "IFCVALVETYPE"),
    class("IFCCABLECARRIERFITTING", "IFCCABLECARRIERFITTINGTYPE"),
    class("IFCCABLEFITTING", "IFCCABLEFITTINGTYPE"),
    class("IFCDUCTFITTING", "IFCDUCTFITTINGTYPE"),
    class("IFCJUNCTIONBOX", "IFCJUNCTIONBOXTYPE"),
    class("IFCPIPEFITTING", "IFCPIPEFITTINGTYPE"),
    class("IFCCOMPRESSOR", "IFCCOMPRESSORTYPE"),
    class("IFCFAN", "IFCFANTYPE"),
    class("IFCPUMP", "IFCPUMPTYPE"),
    class("IFCCABLECARRIERSEGMENT", "IFCCABLECARRIERSEGMENTTYPE"),
    class("IFCCABLESEGMENT", "IFCCABLESEGMENTTYPE"),
    class("IFCDUCTSEGMENT", "IFCDUCTSEGMENTTYPE"),
    class("IFCPIPESEGMENT", "IFCPIPESEGMENTTYPE"),
    class("IFCELECTRICFLOWSTORAGEDEVICE", "IFCELECTRICFLOWSTORAGEDEVICETYPE"),
    class("IFCTANK", "IFCTANKTYPE"),
    class("IFCAIRTERMINAL", "IFCAIRTERMINALTYPE"),
    class("IFCAUDIOVISUALAPPLIANCE", "IFCAUDIOVISUALAPPLIANCETYPE"),
    class("IFCCOMMUNICATIONSAPPLIANCE", "IFCCOMMUNICATIONSAPPLIANCETYPE"),
    class("IFCELECTRICAPPLIANCE", "IFCELECTRICAPPLIANCETYPE"),
    class("IFCFIRESUPPRESSIONTERMINAL", "IFCFIRESUPPRESSIONTERMINALTYPE"),
    class("IFCLAMP", "IFCLAMPTYPE"),
    class("IFCLIGHTFIXTURE", "IFCLIGHTFIXTURETYPE"),
    class("IFCMEDICALDEVICE", "IFCMEDICALDEVICETYPE"),
    class("IFCOUTLET", "IFCOUTLETTYPE"),
    class("IFCSANITARYTERMINAL", "IFCSANITARYTERMINALTYPE"),
    class("IFCSPACEHEATER", "IFCSPACEHEATERTYPE"),
    class("IFCSTACKTERMINAL", "IFCSTACKTERMINALTYPE"),
    class("IFCWASTETERMINAL", "IFCWASTETERMINALTYPE"),
    class("IFCDUCTSILENCER", "IFCDUCTSILENCERTYPE"),
    class("IFCFILTER", "IFCFILTERTYPE"),
    class("IFCINTERCEPTOR", "IFCINTERCEPTORTYPE"),
];

/// How to write occurrences of `entity` (upper case) in IFC4, if it is a
/// physical element class there.
pub fn element_class(entity: &str) -> Option<&'static ElementClass> {
    CLASSES.iter().find(|c| c.entity == entity)
}

/// Readable class name: `IFCPIPESEGMENT` → `IfcPipeSegment` for the classes
/// in the table, else the entity name with only its first letters capitalized.
pub fn display_name(entity: &str) -> String {
    let lower = entity.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("ifc") else { return entity.to_string() };
    // Word boundaries from a small vocabulary of the names above.
    const WORDS: &[&str] = &[
        "standard",
        "elemented",
        "element",
        "proxy",
        "case",
        "covering",
        "curtain",
        "wall",
        "door",
        "window",
        "footing",
        "member",
        "pile",
        "plate",
        "railing",
        "ramp",
        "flight",
        "roof",
        "shading",
        "device",
        "slab",
        "stair",
        "beam",
        "column",
        "chimney",
        "building",
        "civil",
        "assembly",
        "furnishing",
        "furniture",
        "system",
        "geographic",
        "transport",
        "part",
        "discrete",
        "accessory",
        "fastener",
        "mechanical",
        "vibration",
        "isolator",
        "distribution",
        "control",
        "flow",
        "energy",
        "conversion",
        "controller",
        "fitting",
        "moving",
        "segment",
        "storage",
        "terminal",
        "treatment",
        "chamber",
        "actuator",
        "alarm",
        "instrument",
        "protective",
        "tripping",
        "unit",
        "sensor",
        "unitary",
        "air",
        "to",
        "heat",
        "recovery",
        "boiler",
        "burner",
        "chiller",
        "coil",
        "condenser",
        "cooled",
        "cooling",
        "tower",
        "electric",
        "generator",
        "motor",
        "engine",
        "evaporative",
        "cooler",
        "evaporator",
        "exchanger",
        "humidifier",
        "connection",
        "solar",
        "transformer",
        "tube",
        "bundle",
        "equipment",
        "box",
        "damper",
        "board",
        "time",
        "meter",
        "switching",
        "valve",
        "cable",
        "carrier",
        "duct",
        "junction",
        "pipe",
        "compressor",
        "fan",
        "pump",
        "tank",
        "terminal",
        "audio",
        "visual",
        "appliance",
        "communications",
        "fire",
        "suppression",
        "lamp",
        "light",
        "fixture",
        "medical",
        "outlet",
        "sanitary",
        "space",
        "heater",
        "stack",
        "waste",
        "silencer",
        "filter",
        "interceptor",
        "site",
        "opening",
        "virtual",
        "annotation",
        "grid",
        "port",
    ];
    let mut out = String::from("Ifc");
    let mut i = 0;
    while i < rest.len() {
        let word =
            WORDS.iter().filter(|w| rest[i..].starts_with(**w)).max_by_key(|w| w.len()).copied().unwrap_or(&rest[i..]);
        let mut chars = word.chars();
        if let Some(c) = chars.next() {
            out.push(c.to_ascii_uppercase());
            out.extend(chars);
        }
        i += word.len().max(1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_are_unique_and_consistent() {
        for (i, c) in CLASSES.iter().enumerate() {
            assert!(CLASSES[..i].iter().all(|d| d.entity != c.entity), "{} twice", c.entity);
            assert!(c.attrs >= 8 && c.predefined.is_none_or(|p| p >= 8 && p < c.attrs), "{}", c.entity);
        }
        assert_eq!(element_class("IFCDOOR").and_then(|c| c.predefined), Some(10));
        assert!(element_class("IFCELECTRICALELEMENT").is_none());
    }

    #[test]
    fn display_names_are_camel_case() {
        assert_eq!(display_name("IFCPIPESEGMENT"), "IfcPipeSegment");
        assert_eq!(display_name("IFCBUILDINGELEMENTPROXY"), "IfcBuildingElementProxy");
        assert_eq!(display_name("IFCAIRTOAIRHEATRECOVERY"), "IfcAirToAirHeatRecovery");
        assert_eq!(display_name("IFCWALLSTANDARDCASE"), "IfcWallStandardCase");
    }
}
