// Experiment 99: PP-OCR (oar-ocr, ONNX Runtime, CPU) on the 36 real scanned pages of experiment 87.
// Usage: ppocr99 <det.onnx> <rec.onnx> <dict.txt> <out dir> [threads] [limit] [vertical]
// Writes <out>/NNN.txt (one detected line per row) and times.csv (page, size, lines, seconds). Pages already done are skipped.
// With "vertical", pages whose text is mostly in tall boxes (height >= 2 x width) are reordered for vertical writing:
// boxes are grouped into columns by horizontal overlap, columns go right to left, boxes in a column top to bottom.
// Otherwise the library's reading order (top to bottom, left to right) is kept.
use oar_ocr::core::config::OrtSessionConfig;
use oar_ocr::prelude::*;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

struct Line { x0: f32, x1: f32, y0: f32, y1: f32, text: String }

fn vertical_order(mut lines: Vec<Line>) -> Vec<Line> {
    let chars = |l: &Line| l.text.chars().count();
    let total: usize = lines.iter().map(chars).sum();
    let tall: usize = lines.iter().filter(|l| l.y1 - l.y0 >= 2.0 * (l.x1 - l.x0)).map(chars).sum();
    if 2 * tall <= total {
        return lines;
    }
    // columns: right edge first; a box joins a column when it overlaps at least half of the narrower width
    lines.sort_by(|a, b| b.x1.total_cmp(&a.x1));
    let mut columns: Vec<(f32, f32, Vec<Line>)> = Vec::new();
    for line in lines {
        let found = columns.iter_mut().find(|(x0, x1, _)| {
            let overlap = x1.min(line.x1) - x0.max(line.x0);
            overlap >= 0.5 * (x1 - x0).min(line.x1 - line.x0)
        });
        match found {
            Some((x0, x1, members)) => { *x0 = x0.min(line.x0); *x1 = x1.max(line.x1); members.push(line); }
            None => columns.push((line.x0, line.x1, vec![line])),
        }
    }
    columns.sort_by(|a, b| b.1.total_cmp(&a.1));
    columns.into_iter().flat_map(|(_, _, mut members)| { members.sort_by(|a, b| a.y0.total_cmp(&b.y0)); members }).collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = Path::new(&args[4]);
    let threads: usize = args.get(5).map_or(6, |s| s.parse().unwrap());
    let limit: usize = args.get(6).map_or(usize::MAX, |s| s.parse().unwrap());
    let vertical = args.get(7).is_some_and(|s| s == "vertical");
    fs::create_dir_all(out)?;
    let lib = std::env::var("ORT_DYLIB_PATH").map(std::path::PathBuf::from).unwrap_or_else(|_| {
        let dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
        dir.join(if cfg!(windows) { "onnxruntime.dll" } else { "libonnxruntime.so.1.28.0" })
    });
    let _ = ort::init_from(&lib)?.commit();
    // paths in the page list start with /rag (the container mount); RAG_ROOT replaces that prefix (e.g. on Windows)
    let root = std::env::var("RAG_ROOT").unwrap_or_else(|_| "/rag".into());
    let local = |p: &str| format!("{root}{}", p.strip_prefix("/rag").unwrap_or(p));
    let pages: Vec<serde_json::Value> = serde_json::from_str(&fs::read_to_string(local("/rag/work/87_scan_pages.json"))?)?;

    let t = Instant::now();
    let ocr = OAROCRBuilder::new(Path::new(&args[1]), Path::new(&args[2]), Path::new(&args[3]))
        .ort_session(OrtSessionConfig::default().with_intra_threads(threads))
        .build()?;
    println!("load {:.1} s", t.elapsed().as_secs_f64());

    let mut times = fs::OpenOptions::new().create(true).append(true).open(out.join("times.csv"))?;
    for (n, page) in pages.iter().enumerate().take(limit) {
        let file = out.join(format!("{n:03}.txt"));
        if file.exists() {
            continue;
        }
        let image = load_image(Path::new(&local(page["image"].as_str().unwrap())))?;
        let size = format!("{}x{}", image.width(), image.height());
        let t = Instant::now();
        let results = ocr.predict(vec![image])?;
        let seconds = t.elapsed().as_secs_f64();
        let mut lines: Vec<Line> = results[0].recognized_text_regions().map(|r| Line {
            x0: r.bounding_box.x_min(), x1: r.bounding_box.x_max(), y0: r.bounding_box.y_min(), y1: r.bounding_box.y_max(),
            text: r.text.as_deref().unwrap_or("").to_string(),
        }).collect();
        if vertical {
            lines = vertical_order(lines);
        }
        let text = lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
        fs::write(&file, &text)?;
        writeln!(times, "{n},{size},{},{seconds:.2}", lines.len())?;
        println!("{n:03} {size} {} lines {seconds:.1} s {} chars", lines.len(), text.chars().count());
    }
    Ok(())
}
