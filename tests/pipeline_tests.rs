use std::collections::HashMap;

use forge_flux::*;

#[test]
fn pipeline_text_decompose_sort_assemble() {
    let mut p = ForgePipeline::new(
        Box::new(decomposers::TextDecomposer::new()),
        Box::new(assemblers::TextAssembler::new()),
    );
    p.add_transform(Box::new(transforms::SortTransform::new(|a, b| {
        a.payload.cmp(&b.payload)
    })));
    let output = p.run(b"cherry\n\napple\n\nbanana", HashMap::new()).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("apple"));
    assert!(text.contains("banana"));
    assert!(text.contains("cherry"));
}

#[test]
fn pipeline_csv_decompose_filter_assemble() {
    let mut p = ForgePipeline::new(
        Box::new(decomposers::CsvDecomposer::new()),
        Box::new(assemblers::CsvAssembler::new().with_columns(vec![
            "col_name".into(),
            "col_age".into(),
        ])),
    );
    p.add_transform(Box::new(transforms::FilterTransform::new(|tile| {
        tile.meta
            .get("col_age")
            .and_then(|v| v.parse::<u32>().ok())
            .map_or(false, |age| age >= 25)
    })));
    let output = p
        .run(b"name,age\nalice,30\nbob,20\ncharlie,25", HashMap::new())
        .unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("alice"));
    assert!(text.contains("charlie"));
    assert!(!text.contains("bob"));
}

#[test]
fn pipeline_srt_translate_srt() {
    // SubForge pattern: SRT → decompose → translate → assemble SRT
    let mut p = ForgePipeline::new(
        Box::new(decomposers::SubtitleDecomposer::new()),
        Box::new(assemblers::SubtitleAssembler::new(
            assemblers::SubtitleFormat::Srt,
        )),
    );
    p.add_transform(Box::new(transforms::TranslateTransform::new("es")));
    let input = b"1\n00:00:01,000 --> 00:00:04,000\nHello world\n\n2\n00:00:05,000 --> 00:00:08,000\nGoodbye";
    let (output, stages) = p.run_with_tiles(input, HashMap::new()).unwrap();
    assert_eq!(stages.len(), 2); // decompose + translate

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("Hello world"));
    // Translation metadata is on tiles, not in the assembled text
    // Check tiles have translation metadata
    let translated_tiles = &stages[1];
    assert_eq!(translated_tiles[0].meta.get("translated_to").unwrap(), "es");
}

#[test]
fn pipeline_with_tiles_inspection() {
    let p = ForgePipeline::new(
        Box::new(decomposers::TextDecomposer::new()),
        Box::new(assemblers::TextAssembler::new()),
    );
    let (output, stages) = p
        .run_with_tiles(b"hello\n\nworld", HashMap::new())
        .unwrap();
    assert_eq!(stages.len(), 1); // just decompose
    assert_eq!(stages[0].len(), 2);
    let text = String::from_utf8(output).unwrap();
    assert_eq!(text, "hello\n\nworld");
}

#[test]
fn pipeline_map_transform() {
    let mut p = ForgePipeline::new(
        Box::new(decomposers::TextDecomposer::new()),
        Box::new(assemblers::TextAssembler::new()),
    );
    p.add_transform(Box::new(transforms::MapTransform::new(|tile| {
        let s = String::from_utf8_lossy(&tile.payload);
        s.to_uppercase().into_bytes()
    })));
    let output = p.run(b"hello\n\nworld", HashMap::new()).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("HELLO"));
    assert!(text.contains("WORLD"));
}

#[test]
fn pipeline_flux_transform() {
    let mut p = ForgePipeline::new(
        Box::new(decomposers::TextDecomposer::new()),
        Box::new(assemblers::TextAssembler::new()),
    );
    p.add_transform(Box::new(transforms::FluxTransform::new(
        TileKind::Text,
        TileKind::CodeBlock,
    )));
    let (_, stages) = p
        .run_with_tiles(b"fn main() {}", HashMap::new())
        .unwrap();
    let transformed = &stages[1];
    assert_eq!(transformed[0].kind, TileKind::CodeBlock);
}

#[test]
fn conservation_transform_check() {
    let mut p = ForgePipeline::new(
        Box::new(decomposers::TextDecomposer::new()),
        Box::new(assemblers::TextAssembler::new()),
    );
    p.add_transform(Box::new(transforms::ConservationTransform::new(0.5)));
    let (_, stages) = p
        .run_with_tiles(b"hello\n\nworld", HashMap::new())
        .unwrap();
    let checked = &stages[1];
    for tile in checked {
        assert_eq!(tile.meta.get("conservation_checked").unwrap(), "true");
    }
}

#[test]
fn pipeline_single_tile() {
    let p = ForgePipeline::new(
        Box::new(decomposers::TextDecomposer::new()),
        Box::new(assemblers::TextAssembler::new()),
    );
    let output = p.run(b"single paragraph", HashMap::new()).unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), "single paragraph");
}

#[test]
fn pipeline_large_input() {
    let input: String = (0..100).map(|i| format!("Paragraph {i}\n\n")).collect();
    let p = ForgePipeline::new(
        Box::new(decomposers::TextDecomposer::new()),
        Box::new(assemblers::TextAssembler::new()),
    );
    let output = p.run(input.as_bytes(), HashMap::new()).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("Paragraph 0"));
    assert!(text.contains("Paragraph 99"));
}
