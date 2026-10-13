use sourcemap::{SourceMap, SourceMapBuilder as SrcmapBuilder};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A position span in the original DSL source file.
///
/// Intended to match the `SourceSpan` from `crate::ast::SourceSpan` in layout:
/// `file`, `line_start`, `col_start`, `line_end`, `col_end`.
/// Defined here so that the source map layer has zero coupling to the rest of
/// the compiler (parser, codegen) — the codegen layer passes `&SourceSpan`
/// into `record_span`.
#[derive(Debug, Clone)]
pub struct SourceSpan {
    pub file: String,
    pub line_start: u32,
    pub col_start: u32,
    pub line_end: u32,
    pub col_end: u32,
}

/// A single (generated -> source) mapping entry.
#[derive(Debug, Clone)]
pub struct SourceMapping {
    pub generated_line: u32,
    pub generated_col: u32,
    pub source_line: u32,
    pub source_col: u32,
}

/// ECMA-426 compliant source map builder.
///
/// Accumulates mappings during codegen and produces a
/// `//# sourceMappingURL=data:application/json;base64,...` comment string
/// that can be appended to the generated JavaScript output.
pub struct SourceMapBuilder {
    source_name: String,
    generated_name: String,
    inner: SrcmapBuilder,
    mappings: Vec<SourceMapping>,
}

impl SourceMapBuilder {
    /// Create a new builder.
    ///
    /// `source_name`  — original DSL file name (e.g. `"battle.scene"`).
    /// `generated_name` — generated JS file name (e.g. `"battle.scene.js"`).
    pub fn new(source_name: &str, generated_name: &str) -> Self {
        let mut inner = SrcmapBuilder::new(Some(generated_name));
        inner.add_source(source_name);
        Self {
            source_name: source_name.to_owned(),
            generated_name: generated_name.to_owned(),
            inner,
            mappings: Vec::new(),
        }
    }

    /// Record a mapping from a generated JS position back to the original DSL
    /// source position.
    ///
    /// All lines and columns are **0-based** as required by the source map
    /// specification (ECMA-426).
    pub fn add_mapping(
        &mut self,
        generated_line: u32,
        generated_col: u32,
        source_line: u32,
        source_col: u32,
    ) {
        self.inner.add(
            generated_line,
            generated_col,
            source_line,
            source_col,
            Some(self.source_name.as_str()),
            None, // no name
            false,
        );
        self.mappings.push(SourceMapping {
            generated_line,
            generated_col,
            source_line,
            source_col,
        });
    }

    /// Convenience: record a mapping from a [`SourceSpan`] to a position in
    /// the generated JavaScript.
    ///
    /// Only the start of the span (`line_start`, `col_start`) is used; the
    /// end is ignored for mapping purposes (source maps map points, not ranges).
    pub fn record_span(&mut self, span: &SourceSpan, generated_line: u32, generated_col: u32) {
        self.add_mapping(
            generated_line,
            generated_col,
            span.line_start,
            span.col_start,
        );
    }

    /// Finalise the builder and produce the
    /// `//# sourceMappingURL=data:application/json;base64,…` comment string.
    ///
    /// The returned string should be placed at the very end of the generated
    /// JavaScript output (after a trailing newline).
    ///
    /// The builder is consumed by this call.
    pub fn finalize(self) -> String {
        let sm: SourceMap = self.inner.into_sourcemap();
        let data_url = sm
            .to_data_url()
            .expect("sourcemap serialisation should never fail");
        format!("//# sourceMappingURL={data_url}")
    }

    // -- read-only accessors (useful during codegen) -------------------------

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn generated_name(&self) -> &str {
        &self.generated_name
    }

    pub fn mappings(&self) -> &[SourceMapping] {
        &self.mappings
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    /// Helper: extract the JSON payload from a `//# sourceMappingURL=data:...`
    /// comment string and deserialise it into a `sourcemap::SourceMap`.
    fn parse_comment_url(comment: &str) -> SourceMap {
        let data_url = comment
            .strip_prefix("//# sourceMappingURL=")
            .expect("missing sourceMappingURL prefix");

        // data:application/json;charset=utf-8;base64,<payload>
        let payload = data_url
            .strip_prefix("data:application/json;charset=utf-8;base64,")
            .expect("not a base64 data URL");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(payload)
            .expect("base64 decode failed");
        SourceMap::from_reader(decoded.as_slice()).expect("sourcemap parse failed")
    }

    // -----------------------------------------------------------------------
    // 1. Single mapping
    // -----------------------------------------------------------------------
    #[test]
    fn test_add_single_mapping() {
        let mut builder = SourceMapBuilder::new("example.scene", "example.scene.js");
        builder.add_mapping(0, 0, 5, 3);

        let comment = builder.finalize();
        assert!(
            comment.starts_with("//# sourceMappingURL="),
            "output must start with sourceMappingURL comment"
        );
        assert!(
            comment.contains("data:application/json;charset=utf-8;base64,"),
            "must use inline base64 data URL"
        );

        // round-trip through the comment
        let sm = parse_comment_url(&comment);
        assert_eq!(sm.get_token_count(), 1, "expected exactly one token");
    }

    // -----------------------------------------------------------------------
    // 2. Multiple mappings
    // -----------------------------------------------------------------------
    #[test]
    fn test_add_multiple_mappings() {
        let mut builder = SourceMapBuilder::new("multi.scene", "multi.scene.js");
        for i in 0..5u32 {
            builder.add_mapping(i, 0, i * 2, 4);
        }

        let comment = builder.finalize();
        let sm = parse_comment_url(&comment);
        assert_eq!(sm.get_token_count(), 5);
    }

    // -----------------------------------------------------------------------
    // 3. Round-trip: serialise → deserialise → verify every mapping
    // -----------------------------------------------------------------------
    #[test]
    fn test_sourcemap_roundtrip() {
        let input_mappings: Vec<(u32, u32, u32, u32)> = vec![
            (0, 0, 1, 0),
            (0, 5, 1, 10),
            (1, 0, 3, 2),
            (2, 8, 7, 0),
            (5, 0, 12, 0),
        ];

        let mut builder = SourceMapBuilder::new("roundtrip.scene", "roundtrip.scene.js");
        for &(gl, gc, sl, sc) in &input_mappings {
            builder.add_mapping(gl, gc, sl, sc);
        }

        let comment = builder.finalize();
        let sm = parse_comment_url(&comment);

        assert_eq!(sm.get_token_count() as usize, input_mappings.len());

        // Look up each generated position and verify it maps back correctly.
        for (gl, gc, sl, sc) in &input_mappings {
            let token = sm
                .lookup_token(*gl, *gc)
                .unwrap_or_else(|| panic!("no token at ({gl},{gc})"));
            assert_eq!(
                token.get_src_line(),
                *sl,
                "generated ({gl},{gc}) should map to source line {sl}"
            );
            assert_eq!(
                token.get_src_col(),
                *sc,
                "generated ({gl},{gc}) should map to source col {sc}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // 4. Line mapping: generated JS line 42 → DSL source line 5
    // -----------------------------------------------------------------------
    #[test]
    fn test_line_mapping() {
        let mut builder = SourceMapBuilder::new("battle.scene", "battle.scene.js");
        // Simulate codegen: generated JS line 42 corresponds to DSL source line 5
        builder.add_mapping(42, 0, 5, 0);

        let comment = builder.finalize();
        let sm = parse_comment_url(&comment);

        let token = sm
            .lookup_token(42, 0)
            .expect("expected a token at line 42, col 0");
        assert_eq!(token.get_src_line(), 5);
        assert_eq!(token.get_src_col(), 0);
    }

    // -----------------------------------------------------------------------
    // 5. Inline format: verify the comment structure and base64 payload
    // -----------------------------------------------------------------------
    #[test]
    fn test_inline_format() {
        let mut builder = SourceMapBuilder::new("test.scene", "test.scene.js");
        builder.add_mapping(0, 0, 0, 0);

        let comment = builder.finalize();

        // Format: `//# sourceMappingURL=data:application/json;charset=utf-8;base64,<base64>`
        assert!(
            comment.starts_with("//# sourceMappingURL="),
            "must start with the comment prefix"
        );

        let data_url = &comment["//# sourceMappingURL=".len()..];
        assert!(
            data_url.starts_with("data:application/json;charset=utf-8;base64,"),
            "data URL must use correct MIME type"
        );

        let b64_payload = &data_url["data:application/json;charset=utf-8;base64,".len()..];
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(b64_payload)
            .expect("payload is valid base64");

        let json: serde_json::Value =
            serde_json::from_slice(&decoded).expect("payload is valid JSON");
        assert_eq!(json["version"], 3, "ECMA-426 version must be 3");
        assert_eq!(json["sources"][0], "test.scene");
        assert_eq!(json["file"], "test.scene.js");
    }

    // -----------------------------------------------------------------------
    // 6. record_span convenience method
    // -----------------------------------------------------------------------
    #[test]
    fn test_record_span() {
        let span = SourceSpan {
            file: "test.scene".to_owned(),
            line_start: 10,
            col_start: 4,
            line_end: 15,
            col_end: 8,
        };

        let mut builder = SourceMapBuilder::new("test.scene", "test.scene.js");
        builder.record_span(&span, 3, 7);

        let comment = builder.finalize();
        let sm = parse_comment_url(&comment);

        let token = sm.lookup_token(3, 7).expect("expected token at (3,7)");
        assert_eq!(token.get_src_line(), 10);
        assert_eq!(token.get_src_col(), 4);
    }

    // -----------------------------------------------------------------------
    // 7. Empty builder (no mappings)
    // -----------------------------------------------------------------------
    #[test]
    fn test_empty_builder() {
        let builder = SourceMapBuilder::new("empty.scene", "empty.scene.js");
        let comment = builder.finalize();

        let sm = parse_comment_url(&comment);
        assert_eq!(sm.get_token_count(), 0);
    }
}
