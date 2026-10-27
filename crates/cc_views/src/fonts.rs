//! Fallback fonts for the scripts Inter lacks (Chinese, Japanese, Korean),
//! found among the fonts operating systems usually ship and read once.
//! Nothing is bundled, so a system without them shows missing glyphs as
//! boxes, as before.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A font file and the face to use in it (collections hold several).
pub struct FontFile {
    pub data: Vec<u8>,
    pub index: u32,
}

impl FontFile {
    pub fn font_ref(&self) -> Option<skrifa::FontRef<'_>> {
        skrifa::FontRef::from_index(&self.data, self.index).ok()
    }
}

/// Larger files are not fonts we want to hold in memory.
const MAX_FONT_BYTES: u64 = 64 * 1024 * 1024;

/// One character of each script to cover: an ideograph ("中"), kana ("あ")
/// and Hangul ("한").
const PROBES: [char; 3] = ['\u{4E2D}', '\u{3042}', '\u{D55C}'];

/// The system's CJK fonts in fallback order (empty when there is none).
pub fn cjk() -> &'static [FontFile] {
    static CJK: OnceLock<Vec<FontFile>> = OnceLock::new();
    CJK.get_or_init(|| load_covering(&candidates(), &PROBES))
}

/// Readable faces among `candidates`, in order, each kept when it has one
/// of `probes` that no earlier face has; stops once all are covered.
fn load_covering(candidates: &[(PathBuf, u32)], probes: &[char]) -> Vec<FontFile> {
    use skrifa::MetadataProvider;
    let mut missing = probes.to_vec();
    let mut out = Vec::new();
    for (path, index) in candidates {
        if missing.is_empty() {
            break;
        }
        let Some(file) = load(path, *index) else { continue };
        let Some(font) = file.font_ref() else { continue };
        let charmap = font.charmap();
        let before = missing.len();
        missing.retain(|c| charmap.map(*c).is_none());
        if missing.len() < before {
            out.push(file);
        }
    }
    out
}

fn load(path: &Path, index: u32) -> Option<FontFile> {
    if std::fs::metadata(path).ok()?.len() > MAX_FONT_BYTES {
        return None;
    }
    Some(FontFile { data: std::fs::read(path).ok()?, index })
}

/// Usual CJK fonts: Traditional Chinese forms first, then those that add
/// Hangul or kana, then the rest.
fn candidates() -> Vec<(PathBuf, u32)> {
    let mut out = Vec::new();
    if cfg!(windows) {
        let dir = std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let fonts = dir.join("Fonts");
        for name in ["msjh.ttc", "malgun.ttf", "YuGothR.ttc", "meiryo.ttc", "msyh.ttc", "mingliu.ttc", "simsun.ttc"] {
            out.push((fonts.join(name), 0));
        }
    } else if cfg!(target_os = "macos") {
        for name in [
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/AppleSDGothicNeo.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            "/Library/Fonts/Arial Unicode.ttf",
        ] {
            out.push((PathBuf::from(name), 0));
        }
    } else {
        // Noto Sans CJK collections hold JP, KR, SC, TC, HK in that order.
        for name in [
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/google-noto-sans-cjk-fonts/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-VF.ttc",
            "/usr/share/fonts/google-noto-sans-cjk-vf-fonts/NotoSansCJK-VF.ttc",
        ] {
            out.push((PathBuf::from(name), 3));
        }
        for name in [
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/arphic/uming.ttc",
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        ] {
            out.push((PathBuf::from(name), 0));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_readable_faces_that_add_coverage() {
        let fonts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
        let inter = fonts.join("Inter-Regular.ttf");
        let candidates = vec![
            (fonts.join("missing.ttf"), 0),
            // Not a font.
            (fonts.join("OFL-Inter.txt"), 0),
            // A face index past the file's faces.
            (inter.clone(), 5),
            (inter.clone(), 0),
            // Covers nothing new.
            (fonts.join("Inter-SemiBold.ttf"), 0),
        ];
        let found = load_covering(&candidates, &['A', 'B']);
        assert_eq!(found.len(), 1);
        assert!(found[0].font_ref().is_some());
        // Inter has no ideographs, kana or Hangul.
        assert!(load_covering(&candidates, &PROBES).is_empty());
    }
}
