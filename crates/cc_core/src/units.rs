//! Project units: how lengths, areas, volumes, angles, slopes and money are
//! displayed and typed. Values in the model stay in meters and degrees;
//! these settings only affect presentation.

use serde::{Deserialize, Serialize};

/// Meters in an inch (exact, by definition).
pub const INCH: f64 = 0.0254;
/// Meters in a foot (exact, by definition).
pub const FOOT: f64 = 0.3048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum LengthUnit {
    Millimeter,
    Centimeter,
    #[default]
    Meter,
    /// Decimal feet.
    Foot,
    /// Decimal inches.
    Inch,
    /// Feet and fractional inches (`12'-6 1/2"`).
    FeetInches,
    /// Fractional inches (`150 1/2"`).
    FractionalInch,
}

impl LengthUnit {
    pub const ALL: [LengthUnit; 7] = [
        LengthUnit::Millimeter,
        LengthUnit::Centimeter,
        LengthUnit::Meter,
        LengthUnit::Foot,
        LengthUnit::Inch,
        LengthUnit::FeetInches,
        LengthUnit::FractionalInch,
    ];

    /// Display units per meter (feet for feet and inches, inches for
    /// fractional inches: the unit a bare typed number is read in).
    pub fn per_meter(self) -> f64 {
        match self {
            LengthUnit::Millimeter => 1000.0,
            LengthUnit::Centimeter => 100.0,
            LengthUnit::Meter => 1.0,
            LengthUnit::Foot | LengthUnit::FeetInches => 1.0 / FOOT,
            LengthUnit::Inch | LengthUnit::FractionalInch => 1.0 / INCH,
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            LengthUnit::Millimeter => "mm",
            LengthUnit::Centimeter => "cm",
            LengthUnit::Meter => "m",
            LengthUnit::Foot | LengthUnit::FeetInches => "ft",
            LengthUnit::Inch | LengthUnit::FractionalInch => "in",
        }
    }

    /// Shown with fractions of an inch: the decimals setting `d` is then
    /// the precision, 1/2^d of an inch.
    pub fn is_fractional(self) -> bool {
        matches!(self, LengthUnit::FeetInches | LengthUnit::FractionalInch)
    }

    /// A US customary (imperial) unit.
    pub fn is_imperial(self) -> bool {
        matches!(self, LengthUnit::Foot | LengthUnit::Inch | LengthUnit::FeetInches | LengthUnit::FractionalInch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum AreaUnit {
    #[default]
    SquareMeter,
    SquareCentimeter,
    SquareFoot,
    SquareInch,
}

impl AreaUnit {
    pub const ALL: [AreaUnit; 4] =
        [AreaUnit::SquareMeter, AreaUnit::SquareCentimeter, AreaUnit::SquareFoot, AreaUnit::SquareInch];

    pub fn per_square_meter(self) -> f64 {
        match self {
            AreaUnit::SquareMeter => 1.0,
            AreaUnit::SquareCentimeter => 10_000.0,
            AreaUnit::SquareFoot => 1.0 / (FOOT * FOOT),
            AreaUnit::SquareInch => 1.0 / (INCH * INCH),
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            AreaUnit::SquareMeter => "m²",
            AreaUnit::SquareCentimeter => "cm²",
            AreaUnit::SquareFoot => "ft²",
            AreaUnit::SquareInch => "in²",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum VolumeUnit {
    #[default]
    CubicMeter,
    Liter,
    CubicFoot,
    CubicYard,
    /// US liquid gallon.
    Gallon,
}

impl VolumeUnit {
    pub const ALL: [VolumeUnit; 5] =
        [VolumeUnit::CubicMeter, VolumeUnit::Liter, VolumeUnit::CubicFoot, VolumeUnit::CubicYard, VolumeUnit::Gallon];

    pub fn per_cubic_meter(self) -> f64 {
        match self {
            VolumeUnit::CubicMeter => 1.0,
            VolumeUnit::Liter => 1000.0,
            VolumeUnit::CubicFoot => 1.0 / (FOOT * FOOT * FOOT),
            VolumeUnit::CubicYard => 1.0 / (27.0 * FOOT * FOOT * FOOT),
            // 231 cubic inches.
            VolumeUnit::Gallon => 1.0 / (231.0 * INCH * INCH * INCH),
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            VolumeUnit::CubicMeter => "m³",
            VolumeUnit::Liter => "L",
            VolumeUnit::CubicFoot => "ft³",
            VolumeUnit::CubicYard => "yd³",
            VolumeUnit::Gallon => "gal",
        }
    }
}

/// How slopes (rise over run) are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum SlopeUnit {
    #[default]
    Percent,
    Degrees,
}

impl SlopeUnit {
    pub const ALL: [SlopeUnit; 2] = [SlopeUnit::Percent, SlopeUnit::Degrees];

    pub fn symbol(self) -> &'static str {
        match self {
            SlopeUnit::Percent => "%",
            SlopeUnit::Degrees => "°",
        }
    }
}

/// Decimal symbol and digit grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum NumberStyle {
    /// Decimal symbol of the interface language, no grouping.
    #[default]
    Language,
    /// 1.234.567,89
    CommaDecimalPointGroups,
    /// 1 234 567,89
    CommaDecimalSpaceGroups,
    /// 1234567,89
    CommaDecimal,
    /// 1,234,567.89
    PointDecimalCommaGroups,
    /// 1234567.89
    PointDecimal,
}

impl NumberStyle {
    pub const ALL: [NumberStyle; 6] = [
        NumberStyle::Language,
        NumberStyle::CommaDecimalPointGroups,
        NumberStyle::CommaDecimalSpaceGroups,
        NumberStyle::CommaDecimal,
        NumberStyle::PointDecimalCommaGroups,
        NumberStyle::PointDecimal,
    ];

    /// Decimal symbol and group separator, given the language's decimal comma.
    pub fn separators(self, language_comma: bool) -> (char, Option<char>) {
        match self {
            NumberStyle::Language => (if language_comma { ',' } else { '.' }, None),
            NumberStyle::CommaDecimalPointGroups => (',', Some('.')),
            NumberStyle::CommaDecimalSpaceGroups => (',', Some('\u{202f}')),
            NumberStyle::CommaDecimal => (',', None),
            NumberStyle::PointDecimalCommaGroups => ('.', Some(',')),
            NumberStyle::PointDecimal => ('.', None),
        }
    }
}

/// Most decimals any unit may show.
pub const MAX_DECIMALS: u8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Format<U> {
    pub unit: U,
    pub decimals: u8,
    /// Append the unit symbol (dimensions usually omit it).
    pub symbol: bool,
}

/// Display settings for every kind of quantity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Units {
    pub length: Format<LengthUnit>,
    pub area: Format<AreaUnit>,
    pub volume: Format<VolumeUnit>,
    /// Angles are always degrees.
    pub angle: Format<()>,
    pub slope: Format<SlopeUnit>,
    /// Currency symbol written before amounts (empty for none).
    pub currency: String,
    pub currency_decimals: u8,
    pub style: NumberStyle,
}

impl Default for Units {
    fn default() -> Self {
        Self {
            length: Format { unit: LengthUnit::Meter, decimals: 2, symbol: false },
            area: Format { unit: AreaUnit::SquareMeter, decimals: 2, symbol: true },
            volume: Format { unit: VolumeUnit::CubicMeter, decimals: 3, symbol: true },
            angle: Format { unit: (), decimals: 2, symbol: true },
            slope: Format { unit: SlopeUnit::Percent, decimals: 1, symbol: true },
            currency: String::new(),
            currency_decimals: 2,
            style: NumberStyle::Language,
        }
    }
}

impl Units {
    /// US customary units: feet and inches to 1/16", square feet, cubic
    /// feet, point decimals with comma groups.
    pub fn imperial() -> Self {
        Self {
            length: Format { unit: LengthUnit::FeetInches, decimals: 4, symbol: false },
            area: Format { unit: AreaUnit::SquareFoot, decimals: 2, symbol: true },
            volume: Format { unit: VolumeUnit::CubicFoot, decimals: 2, symbol: true },
            style: NumberStyle::PointDecimalCommaGroups,
            ..Self::default()
        }
    }

    /// Clamp decimals to [`MAX_DECIMALS`]; returns whether anything changed.
    pub fn repair(&mut self) -> bool {
        let mut changed = false;
        for d in [
            &mut self.length.decimals,
            &mut self.area.decimals,
            &mut self.volume.decimals,
            &mut self.angle.decimals,
            &mut self.slope.decimals,
            &mut self.currency_decimals,
        ] {
            if *d > MAX_DECIMALS {
                *d = MAX_DECIMALS;
                changed = true;
            }
        }
        if self.currency.chars().count() > 8 {
            self.currency = self.currency.chars().take(8).collect();
            changed = true;
        }
        changed
    }
}
