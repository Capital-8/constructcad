//! Vector export of drawings in paper millimeters (composed sheets) to SVG
//! and PDF. Both writers are self-contained: the PDF uses the standard
//! Helvetica font, so no font is embedded.

use crate::{Anchor, Drawing2D, Prim};
use glam::DVec2;
use std::fmt::Write as _;

const TEXT_GRAY: u8 = 20;

/// Baseline offset (in text heights) from the anchor point, and the
/// horizontal fraction of the text width left of it.
fn anchor_offsets(anchor: Anchor) -> (f64, f64) {
    match anchor {
        Anchor::LeftBottom => (0.28, 0.0),
        Anchor::CenterBottom => (0.28, 0.5),
        Anchor::Center => (-0.35, 0.5),
    }
}

fn num(v: f64) -> String {
    let v = if v.is_finite() { v } else { 0.0 };
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() { "0".to_string() } else { s.to_string() }
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if (c as u32) < 0x20 && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

/// SVG document of `drawing` on a sheet of `paper` millimeters.
pub fn to_svg(drawing: &Drawing2D, paper: DVec2) -> String {
    let (w, h) = (paper.x, paper.y);
    let y = |v: f64| num(h - v);
    let mut s = String::new();
    let _ = writeln!(s, r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    let _ = writeln!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{0}mm" height="{1}mm" viewBox="0 0 {0} {1}">"#,
        num(w),
        num(h)
    );
    let _ = writeln!(s, r#"<rect width="{}" height="{}" fill="white"/>"#, num(w), num(h));
    let _ = writeln!(s, r#"<g stroke-linecap="round" fill="none">"#);
    for prim in &drawing.prims {
        match prim {
            Prim::Line { a, b, style, pen, .. } => {
                let resolved = style.resolve(pen.as_ref());
                let (width, [r, g, bl]) = (resolved.width, resolved.color);
                let dash = resolved.dashes;
                let dash = if dash.is_empty() {
                    String::new()
                } else {
                    let d: Vec<String> = dash.iter().map(|v| num(*v)).collect();
                    format!(r#" stroke-dasharray="{}""#, d.join(" "))
                };
                let _ = writeln!(
                    s,
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="rgb({r},{g},{bl})" stroke-width="{}"{dash}/>"#,
                    num(a.x),
                    y(a.y),
                    num(b.x),
                    y(b.y),
                    num(width)
                );
            }
            Prim::Fill { pts, color, .. } => {
                let [r, g, b, a] = *color;
                if a == 0 || pts.len() < 3 {
                    continue;
                }
                let points: Vec<String> = pts.iter().map(|p| format!("{},{}", num(p.x), y(p.y))).collect();
                let opacity =
                    if a == 255 { String::new() } else { format!(r#" fill-opacity="{}""#, num(a as f64 / 255.0)) };
                let _ = writeln!(
                    s,
                    r#"<polygon points="{}" fill="rgb({r},{g},{b})"{opacity} stroke="none"/>"#,
                    points.join(" ")
                );
            }
            Prim::Text { pos, text, size, anchor, angle, .. } => {
                if text.trim().is_empty() {
                    continue;
                }
                let (base, frac) = anchor_offsets(*anchor);
                let text_anchor = if frac == 0.0 { "start" } else { "middle" };
                // Offset the baseline in the text's own (rotated) frame.
                let up = DVec2::new(-angle.sin(), angle.cos());
                let p = *pos - up * (base * size);
                let rotate = if angle.abs() > 1e-9 {
                    format!(r#" transform="rotate({} {} {})""#, num(-angle.to_degrees()), num(p.x), y(p.y))
                } else {
                    String::new()
                };
                let _ = writeln!(
                    s,
                    r#"<text x="{}" y="{}" font-family="Helvetica, Arial, sans-serif" font-size="{}" text-anchor="{text_anchor}" fill="rgb({g},{g},{g})" stroke="none"{rotate}>{}</text>"#,
                    num(p.x),
                    y(p.y),
                    num(*size),
                    xml_escape(text),
                    g = TEXT_GRAY
                );
            }
        }
    }
    s.push_str("</g>\n</svg>\n");
    s
}

/// Advance widths of Helvetica for ASCII 32..=126, in 1/1000 em.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556,
    556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833,
    722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556,
    556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334,
    260, 334, 584,
];

/// Byte of `c` in WinAnsiEncoding (`?` when it has none).
fn win_ansi(c: char) -> u8 {
    let code = c as u32;
    match c {
        '€' => 0x80,
        '‚' => 0x82,
        '„' => 0x84,
        '…' => 0x85,
        '‘' => 0x91,
        '’' => 0x92,
        '“' => 0x93,
        '”' => 0x94,
        '•' => 0x95,
        '–' => 0x96,
        '—' => 0x97,
        _ if (0x20..0x7f).contains(&code) || (0xa0..=0xff).contains(&code) => code as u8,
        _ => b'?',
    }
}

/// Width of `bytes` (WinAnsi) set in Helvetica, in ems.
fn helvetica_width(bytes: &[u8]) -> f64 {
    bytes
        .iter()
        .map(|b| match b {
            0x20..=0x7e => HELVETICA[(b - 0x20) as usize] as f64,
            0x96 => 556.0,
            0x97 => 1000.0,
            0x85 => 1000.0,
            _ => 556.0,
        })
        .sum::<f64>()
        / 1000.0
}

fn pdf_string(bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![b'('];
    for &b in bytes {
        if matches!(b, b'(' | b')' | b'\\') {
            out.push(b'\\');
        }
        out.push(b);
    }
    out.push(b')');
    out
}

fn rgb(c: [u8; 3]) -> String {
    format!("{} {} {}", num(c[0] as f64 / 255.0), num(c[1] as f64 / 255.0), num(c[2] as f64 / 255.0))
}

/// PDF document (one page) of `drawing` on a sheet of `paper` millimeters.
pub fn to_pdf(drawing: &Drawing2D, paper: DVec2) -> Vec<u8> {
    const PT_PER_MM: f64 = 72.0 / 25.4;
    let mut alphas: Vec<u8> = Vec::new();
    let mut content: Vec<u8> = Vec::new();
    // Work in millimeters with the origin at the bottom-left corner.
    let _ = writeln!(Utf8(&mut content), "{} 0 0 {} 0 0 cm 1 J 1 j", num(PT_PER_MM), num(PT_PER_MM));
    for prim in &drawing.prims {
        match prim {
            Prim::Line { a, b, style, pen, .. } => {
                let resolved = style.resolve(pen.as_ref());
                let (width, color) = (resolved.width, resolved.color);
                let dash: Vec<String> = resolved.dashes.iter().map(|v| num(*v)).collect();
                let _ = writeln!(
                    Utf8(&mut content),
                    "{} RG {} w [{}] 0 d {} {} m {} {} l S",
                    rgb(color),
                    num(width),
                    dash.join(" "),
                    num(a.x),
                    num(a.y),
                    num(b.x),
                    num(b.y)
                );
            }
            Prim::Fill { pts, color, .. } => {
                let [r, g, b, a] = *color;
                if a == 0 || pts.len() < 3 {
                    continue;
                }
                let gs = if a == 255 {
                    String::new()
                } else {
                    let i = alphas.iter().position(|v| *v == a).unwrap_or_else(|| {
                        alphas.push(a);
                        alphas.len() - 1
                    });
                    format!("/GS{i} gs ")
                };
                let mut path = String::new();
                for (i, p) in pts.iter().enumerate() {
                    let _ = write!(path, "{} {} {} ", num(p.x), num(p.y), if i == 0 { "m" } else { "l" });
                }
                let _ = writeln!(Utf8(&mut content), "q {gs}{} rg {path}h f Q", rgb([r, g, b]));
            }
            Prim::Text { pos, text, size, anchor, angle, .. } => {
                let bytes: Vec<u8> = text.chars().map(win_ansi).collect();
                if text.trim().is_empty() {
                    continue;
                }
                let (base, frac) = anchor_offsets(*anchor);
                let width = helvetica_width(&bytes) * size;
                let (sin, cos) = angle.sin_cos();
                let along = DVec2::new(cos, sin);
                let up = DVec2::new(-sin, cos);
                let p = *pos - along * (width * frac) - up * (base * size);
                let g = num(TEXT_GRAY as f64 / 255.0);
                let _ = write!(
                    Utf8(&mut content),
                    "BT {g} g /F1 {} Tf {} {} {} {} {} {} Tm ",
                    num(*size),
                    num(cos),
                    num(sin),
                    num(-sin),
                    num(cos),
                    num(p.x),
                    num(p.y)
                );
                content.extend(pdf_string(&bytes));
                content.extend(b" Tj ET\n");
            }
        }
    }

    let (w, h) = (paper.x * PT_PER_MM, paper.y * PT_PER_MM);
    let mut ext = String::new();
    for (i, a) in alphas.iter().enumerate() {
        let _ = write!(ext, "/GS{i} << /Type /ExtGState /ca {} >> ", num(*a as f64 / 255.0));
    }
    let mut objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 5 0 R >> /ExtGState << {ext}>> >> /Contents 4 0 R >>",
            num(w),
            num(h)
        )
        .into_bytes(),
    ];
    let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
    stream.extend(&content);
    stream.extend(b"\nendstream");
    objects.push(stream);
    objects.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec());
    objects.push(b"<< /Producer (ConstructCAD) >>".to_vec());

    let mut out: Vec<u8> = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", i + 1).into_bytes());
        out.extend(body);
        out.extend(b"\nendobj\n");
    }
    let xref = out.len();
    let mut tail = format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1);
    for o in offsets {
        let _ = writeln!(tail, "{o:010} 00000 n ");
    }
    let _ = write!(
        tail,
        "trailer\n<< /Size {} /Root 1 0 R /Info {} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1,
        objects.len()
    );
    out.extend(tail.into_bytes());
    out
}

/// `fmt::Write` adapter for building ASCII content into a byte buffer.
struct Utf8<'a>(&'a mut Vec<u8>);

impl std::fmt::Write for Utf8<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.extend_from_slice(s.as_bytes());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LineStyle;

    fn sample() -> Drawing2D {
        let mut d = Drawing2D::default();
        d.line(DVec2::new(10.0, 10.0), DVec2::new(100.0, 10.0), LineStyle::Cut, None);
        d.line(DVec2::new(10.0, 20.0), DVec2::new(100.0, 20.0), LineStyle::Hidden, None);
        d.fill(vec![DVec2::ZERO, DVec2::new(5.0, 0.0), DVec2::new(5.0, 5.0)], [200, 0, 0, 128], None);
        d.text_at(DVec2::new(50.0, 50.0), "Planta <Térreo> & (cobertura) — 1:50", 3.0, Anchor::Center, 0.3);
        d
    }

    #[test]
    fn svg_has_every_primitive() {
        let svg = to_svg(&sample(), DVec2::new(420.0, 297.0));
        assert!(svg.contains(r#"width="420mm" height="297mm" viewBox="0 0 420 297""#));
        assert_eq!(svg.matches("<line ").count(), 2);
        assert!(svg.contains("stroke-dasharray"));
        assert!(svg.contains("fill-opacity"));
        assert!(svg.contains("Planta &lt;Térreo&gt; &amp; (cobertura) — 1:50"));
        assert!(svg.contains("rotate("));
        // Paper y is flipped: the line at y = 10 sits at 287 from the top.
        assert!(svg.contains(r#"y1="287""#));
    }

    #[test]
    fn pdf_structure_is_consistent() {
        let pdf = to_pdf(&sample(), DVec2::new(420.0, 297.0));
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        let find = |needle: &[u8]| pdf.windows(needle.len()).position(|w| w == needle).unwrap();
        let rfind = |needle: &[u8]| pdf.windows(needle.len()).rposition(|w| w == needle).unwrap();
        let ascii = |a: usize, b: usize| String::from_utf8(pdf[a..b].to_vec()).unwrap();
        // startxref points at the xref table.
        let sx = rfind(
            b"startxref
",
        ) + 10;
        let start: usize = ascii(sx, pdf.len()).lines().next().unwrap().parse().unwrap();
        assert!(pdf[start..].starts_with(b"xref"));
        // Every object offset in the table points at its "N 0 obj".
        let table = ascii(start, pdf.len());
        for (i, line) in table.lines().skip(3).take(6).enumerate() {
            let off: usize = line[..10].parse().unwrap();
            assert!(pdf[off..].starts_with(format!("{} 0 obj", i + 1).as_bytes()), "object {}", i + 1);
        }
        // Stream length matches its content.
        let s = find(
            b"stream
",
        ) + 7;
        let e = find(
            b"
endstream",
        );
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains(&format!("/Length {}", e - s)));
        assert!(text.contains("/GS0 gs"));
        assert!(text.contains("1190.551 841.89"));
        // Parentheses are escaped; the em dash is WinAnsi 0x97.
        assert!(pdf.windows(2).any(|w| w == b"\\("));
        assert!(pdf.contains(&0x97));
    }
}
