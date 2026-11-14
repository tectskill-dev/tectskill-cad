//! Reading AutoCAD plot style table files (`.ctb` colour-dependent, `.stb` named) into a
//! [`PlotStyleTable`], so an office's existing pen settings carry over.
//!
//! The format, as publicly documented by open-source readers: a 60-byte header that starts
//! `PIAFILEVERSION_2.0,CTBVER1,compress` (or `STBVER1`), then a zlib stream holding text of
//! `key=value` lines and `name{ ... }` blocks: a `description`, a `custom_lineweight_table` of
//! lineweights in millimetres by index, and a `plot_style` block with one numbered block per
//! style (`name`, `color`, `color_policy`, `screen`, `linetype`, `lineweight`, `end_style`,
//! `join_style`, `fill_style`). Every number is input from an outside file: parsed with checks,
//! never indexed directly, and the inflated text is capped.

use std::collections::BTreeMap;

use cadcraft_color::Rgb;

use crate::{EndStyle, FillStyle, JoinStyle, MAX_NAMED_STYLES, MAX_TABLE_FILE, PlotStyle, PlotStyleKind, PlotStyleTable};

/// What an imported table could not carry over.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AcadImportNotes {
    /// Styles (colour numbers for `.ctb`, names for `.stb`) whose AutoCAD plot linetype is
    /// neither "use object linetype" nor solid: they keep the object's linetype.
    pub linetypes_not_converted: Vec<String>,
}

/// Header every AutoCAD plot style table file starts with.
const MAGIC: &[u8] = b"PIAFILEVERSION";
/// Bytes before the compressed text.
const HEADER_LEN: usize = 60;
/// Colour value meaning "use the object's colour".
const OBJECT_COLOR: i64 = -1_006_632_961;
/// Index values meaning "use the object's" linetype / lineweight.
const OBJECT_LINETYPE: i64 = 31;
const OBJECT_LINEWEIGHT: i64 = 255;
/// The lineweight list AutoCAD tables use when they carry none of their own, in millimetres.
const DEFAULT_LINEWEIGHTS: [f64; 27] = [
    0.0, 0.05, 0.09, 0.10, 0.13, 0.15, 0.18, 0.20, 0.25, 0.30, 0.35, 0.40, 0.45, 0.50, 0.53, 0.60, 0.65, 0.70, 0.80, 0.90, 1.00, 1.06, 1.20, 1.40,
    1.58, 2.00, 2.11,
];
/// Nesting depth the text parser follows (real files use two levels).
const MAX_DEPTH: usize = 8;

/// Whether `bytes` look like an AutoCAD plot style table file.
pub fn is_acad_plot_style_file(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

/// Read an AutoCAD `.ctb` / `.stb` file. `file_name` names the table (its extension decides the
/// kind when the header does not).
pub fn plot_style_table_from_acad(file_name: &str, bytes: &[u8]) -> Result<(PlotStyleTable, AcadImportNotes), String> {
    if !is_acad_plot_style_file(bytes) {
        return Err("not an AutoCAD plot style table (.ctb / .stb)".into());
    }
    let head = String::from_utf8_lossy(bytes.get(..HEADER_LEN.min(bytes.len())).unwrap_or_default()).to_ascii_uppercase();
    let lower = file_name.to_ascii_lowercase();
    let named = head.contains("STBVER") || (!head.contains("CTBVER") && lower.ends_with(".stb"));
    let text = inflate(bytes)?;
    let tree = parse(&text);

    let weights: Vec<f64> = match tree.block("custom_lineweight_table") {
        Some(b) => {
            let mut w: Vec<(usize, f64)> =
                b.values.iter().filter_map(|(k, v)| Some((k.parse::<usize>().ok()?, v.parse::<f64>().ok()?))).filter(|(i, _)| *i < 256).collect();
            w.sort_by_key(|(i, _)| *i);
            let mut out = vec![f64::NAN; w.last().map_or(0, |(i, _)| i + 1)];
            for (i, v) in w {
                if let Some(slot) = out.get_mut(i) {
                    *slot = v;
                }
            }
            out
        }
        None => DEFAULT_LINEWEIGHTS.to_vec(),
    };

    let mut raw: Vec<(usize, &Node)> =
        tree.block("plot_style").map(|b| b.blocks.iter().filter_map(|(k, n)| Some((k.parse::<usize>().ok()?, n))).collect()).unwrap_or_default();
    raw.sort_by_key(|(i, _)| *i);

    let mut notes = AcadImportNotes::default();
    let name = table_name(file_name, named);
    let description = tree.value("description").unwrap_or_default().to_string();
    let table = if named {
        let styles: Vec<PlotStyle> = raw
            .iter()
            .take(MAX_NAMED_STYLES)
            .map(|(_, n)| {
                let label = n.value("name").unwrap_or("Style").to_string();
                let mut st = style(n, &weights, &mut notes, &label);
                st.name = label;
                st
            })
            .collect();
        PlotStyleTable { name, description, kind: PlotStyleKind::Named, styles }.sanitized()
    } else {
        let by_index: BTreeMap<usize, &Node> = raw.into_iter().collect();
        let mut styles = Vec::with_capacity(255);
        for i in 0..255usize {
            let label = (i + 1).to_string();
            let st = match by_index.get(&i) {
                Some(n) => style(n, &weights, &mut notes, &label),
                None => PlotStyle::object(""),
            };
            styles.push(st);
        }
        PlotStyleTable { name, description, kind: PlotStyleKind::ColorDependent, styles }.sanitized()
    };
    Ok((table, notes))
}

fn table_name(file_name: &str, named: bool) -> String {
    let base = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name).trim();
    let lower = base.to_ascii_lowercase();
    if lower.ends_with(".ctb") || lower.ends_with(".stb") {
        base.to_string()
    } else if base.is_empty() {
        if named { "imported.stb".into() } else { "imported.ctb".into() }
    } else {
        format!("{base}{}", if named { ".stb" } else { ".ctb" })
    }
}

fn int(n: &Node, key: &str) -> Option<i64> {
    n.value(key).and_then(|v| v.trim().parse::<i64>().ok())
}

fn style(n: &Node, weights: &[f64], notes: &mut AcadImportNotes, label: &str) -> PlotStyle {
    let mut st = PlotStyle::object("");
    if let Some(d) = n.value("description").filter(|d| !d.is_empty()) {
        st.description = d.to_string();
    }
    let color = int(n, "color").unwrap_or(OBJECT_COLOR);
    if color != OBJECT_COLOR && color != -1 {
        let c = int(n, "mode_color").unwrap_or(color);
        st.color = Some(Rgb::from_u32((c & 0x00ff_ffff) as u32));
    }
    if int(n, "color_policy").is_some_and(|p| p & 2 != 0) {
        st.grayscale = true;
    }
    if let Some(s) = int(n, "screen").filter(|s| (0..100).contains(s)) {
        st.screening = s as u8;
    }
    match int(n, "linetype").unwrap_or(OBJECT_LINETYPE) {
        OBJECT_LINETYPE => {}
        0 => st.linetype = Some("Continuous".into()),
        _ => notes.linetypes_not_converted.push(label.to_string()),
    }
    let lw = int(n, "lineweight").unwrap_or(OBJECT_LINEWEIGHT);
    if lw != OBJECT_LINEWEIGHT
        && let Some(mm) = usize::try_from(lw).ok().and_then(|i| weights.get(i)).copied().filter(|v| v.is_finite() && *v >= 0.0)
    {
        st.lineweight = Some(((mm * 100.0).round() as u16).min(211));
    }
    st.end = match int(n, "end_style") {
        Some(0) => EndStyle::Butt,
        Some(1) => EndStyle::Square,
        Some(2) => EndStyle::Round,
        Some(3) => EndStyle::Diamond,
        _ => EndStyle::default(),
    };
    st.join = match int(n, "join_style") {
        Some(0) => JoinStyle::Miter,
        Some(1) => JoinStyle::Bevel,
        Some(2) => JoinStyle::Round,
        Some(3) => JoinStyle::Diamond,
        _ => JoinStyle::default(),
    };
    st.fill = match int(n, "fill_style") {
        Some(64) => FillStyle::Solid,
        Some(65) => FillStyle::Checkerboard,
        Some(66) => FillStyle::Crosshatch,
        Some(67) => FillStyle::Diamonds,
        Some(68) => FillStyle::HorizontalBars,
        Some(69) => FillStyle::SlantLeft,
        Some(70) => FillStyle::SlantRight,
        Some(71) => FillStyle::SquareDots,
        Some(72) => FillStyle::VerticalBars,
        _ => FillStyle::default(),
    };
    st
}

/// The table's text: the zlib stream after the header (searched for when a writer used a
/// different header length), inflated up to [`MAX_TABLE_FILE`].
fn inflate(bytes: &[u8]) -> Result<String, String> {
    let start = if bytes.get(HEADER_LEN) == Some(&0x78) {
        HEADER_LEN
    } else {
        (40..bytes.len().min(160).saturating_sub(1))
            .find(|&i| bytes.get(i) == Some(&0x78) && matches!(bytes.get(i + 1), Some(0x01 | 0x5e | 0x9c | 0xda)))
            .ok_or("the plot style table file is damaged (no compressed data)")?
    };
    let data = bytes.get(start..).unwrap_or_default();
    let out = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(data, MAX_TABLE_FILE)
        .map_err(|_| "the plot style table file is damaged or too large".to_string())?;
    let end = out.iter().rposition(|b| *b != 0).map_or(0, |i| i + 1);
    let body = out.get(..end).unwrap_or_default();
    Ok(match std::str::from_utf8(body) {
        Ok(s) => s.to_string(),
        Err(_) => decode_ansi(body),
    })
}

/// Text written in a Windows code page: Thai (874) letters map to U+0E01.., other high bytes to
/// Latin-1, so a Thai description still reads.
fn decode_ansi(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| match b {
            0xA1..=0xFB => char::from_u32(0x0E01 + u32::from(b - 0xA1)).unwrap_or('?'),
            _ => char::from(b),
        })
        .collect()
}

#[derive(Default, Debug)]
struct Node {
    values: Vec<(String, String)>,
    blocks: Vec<(String, Node)>,
}

impl Node {
    fn value(&self, key: &str) -> Option<&str> {
        self.values.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
    fn block(&self, key: &str) -> Option<&Node> {
        self.blocks.iter().find(|(k, _)| k == key).map(|(_, n)| n)
    }
}

fn parse(text: &str) -> Node {
    let mut stack: Vec<(String, Node)> = vec![(String::new(), Node::default())];
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if line == "}" {
            if stack.len() > 1
                && let Some((k, n)) = stack.pop()
                && let Some((_, parent)) = stack.last_mut()
            {
                parent.blocks.push((k, n));
            }
        } else if let Some(name) = line.strip_suffix('{').filter(|_| !line.contains('=')) {
            if stack.len() <= MAX_DEPTH {
                stack.push((name.trim().to_string(), Node::default()));
            }
        } else if let Some((k, v)) = line.split_once('=')
            && let Some((_, cur)) = stack.last_mut()
        {
            let v = v.trim();
            // Strings start with a quote and run to the end of the line.
            let v = match v.strip_prefix('"') {
                Some(s) => s.strip_suffix('"').unwrap_or(s),
                None => v,
            };
            cur.values.push((k.trim().to_string(), v.to_string()));
        }
    }
    while stack.len() > 1 {
        if let Some((k, n)) = stack.pop()
            && let Some((_, parent)) = stack.last_mut()
        {
            parent.blocks.push((k, n));
        }
    }
    stack.pop().map(|(_, n)| n).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(header: &str, text: &str) -> Vec<u8> {
        let mut body = text.as_bytes().to_vec();
        body.push(0);
        let comp = miniz_oxide::deflate::compress_to_vec_zlib(&body, 6);
        let mut out = header.as_bytes().to_vec();
        out.resize(HEADER_LEN, 0);
        out.extend_from_slice(&comp);
        out
    }

    fn ctb_text() -> String {
        let mut t = String::from(
            "description=\"Office pens\ncustom_lineweight_table{\n0=0.00\n1=0.05\n2=0.09\n3=0.13\n4=0.18\n5=0.25\n6=0.35\n}\nplot_style{\n",
        );
        for i in 0..255 {
            let (color, lw, lt, screen) = match i {
                0 => (-16_777_216i64, 6, 31, 100),
                1 => (OBJECT_COLOR, 4, 31, 50),
                2 => (-1_023_475_712, 255, 5, 100),
                _ => (OBJECT_COLOR, 255, 31, 100),
            };
            t.push_str(&format!(
                " {i}{{\n  name=\"Color_{}\n  color={color}\n  color_policy={}\n  screen={screen}\n  linetype={lt}\n  lineweight={lw}\n  end_style={}\n  join_style=5\n  fill_style=73\n }}\n",
                i + 1,
                if i == 1 { 3 } else { 1 },
                if i == 2 { 2 } else { 4 }
            ));
        }
        t.push_str("}\n");
        t
    }

    #[test]
    fn reads_a_color_dependent_table() {
        let bytes = file("PIAFILEVERSION_2.0,CTBVER1,compress\r\npmzlibcodec", &ctb_text());
        let (t, notes) = plot_style_table_from_acad("C:\\pens\\office.ctb", &bytes).unwrap();
        assert_eq!(t.name, "office.ctb");
        assert_eq!(t.description, "Office pens");
        assert!(t.is_color_dependent());
        assert_eq!(t.styles.len(), 255);
        let s1 = t.by_color(1).unwrap();
        assert_eq!(s1.color, Some(Rgb(0, 0, 0)));
        assert_eq!(s1.lineweight, Some(35));
        let s2 = t.by_color(2).unwrap();
        assert_eq!(s2.color, None);
        assert!(s2.grayscale);
        assert_eq!(s2.screening, 50);
        assert_eq!(s2.lineweight, Some(18));
        let s3 = t.by_color(3).unwrap();
        assert_eq!(s3.color, Some(Rgb(255, 0, 0)));
        assert_eq!(s3.lineweight, None);
        assert_eq!(s3.end, EndStyle::Round);
        assert_eq!(t.by_color(9).unwrap().lineweight, None);
        assert_eq!(notes.linetypes_not_converted, vec!["3".to_string()]);
    }

    #[test]
    fn reads_a_named_table_and_keeps_normal_first() {
        let text = "description=\"named\nplot_style{\n 0{\n  name=\"Normal\n  color=-1006632961\n  lineweight=255\n }\n 1{\n  name=\"Pen 5\n  color=-16777216\n  lineweight=13\n }\n}\n";
        let bytes = file("PIAFILEVERSION_2.0,STBVER1,compress\r\npmzlibcodec", text);
        let (t, _) = plot_style_table_from_acad("office.stb", &bytes).unwrap();
        assert!(!t.is_color_dependent());
        assert_eq!(t.styles.first().unwrap().name, "Normal");
        let pen = t.by_name("Pen 5").unwrap();
        assert_eq!(pen.lineweight, Some(50));
        assert_eq!(pen.color, Some(Rgb(0, 0, 0)));
    }

    #[test]
    fn hostile_files_are_errors_not_panics() {
        assert!(plot_style_table_from_acad("x.ctb", b"").is_err());
        assert!(plot_style_table_from_acad("x.ctb", b"PIAFILEVERSION").is_err());
        let mut junk = b"PIAFILEVERSION_2.0,CTBVER1,compress".to_vec();
        junk.resize(HEADER_LEN, 0);
        junk.extend_from_slice(&[0x78, 0x9c, 0xff, 0xff, 0x00]);
        assert!(plot_style_table_from_acad("x.ctb", &junk).is_err());
        // Huge indices and garbage numbers are ignored.
        let text = "plot_style{\n 99999999999999999999{\n  color=abc\n }\n 4{\n  lineweight=-7\n  screen=100000\n }\n";
        let (t, _) = plot_style_table_from_acad("x.ctb", &file("PIAFILEVERSION_2.0,CTBVER1,compress", text)).unwrap();
        assert_eq!(t.styles.len(), 255);
        assert_eq!(t.by_color(5).unwrap().lineweight, None);
    }

    #[test]
    fn deep_nesting_is_bounded() {
        let text = "a{\n".repeat(10_000);
        let (t, _) = plot_style_table_from_acad("x.ctb", &file("PIAFILEVERSION_2.0,CTBVER1,compress", &text)).unwrap();
        assert_eq!(t.styles.len(), 255);
    }

    #[test]
    fn thai_ansi_description_reads() {
        let mut text = b"description=\"".to_vec();
        text.extend_from_slice(&[0xBB, 0xD2, 0xA1, 0xA1, 0xD2]); // ปากกา in code page 874
        text.extend_from_slice(b"\nplot_style{\n}\n");
        let mut body = text.clone();
        body.push(0);
        let mut bytes = b"PIAFILEVERSION_2.0,CTBVER1,compress".to_vec();
        bytes.resize(HEADER_LEN, 0);
        bytes.extend_from_slice(&miniz_oxide::deflate::compress_to_vec_zlib(&body, 6));
        let (t, _) = plot_style_table_from_acad("x.ctb", &bytes).unwrap();
        assert_eq!(t.description, "ปากกา");
    }
}
