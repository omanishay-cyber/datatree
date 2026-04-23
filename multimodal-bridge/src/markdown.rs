//! Pure-Rust Markdown extractor via `pulldown-cmark`.
//!
//! Produces:
//! * `pages` — one entry per top-level (h1/h2) section, so `media_fts`
//!   can surface per-section hits.
//! * `elements` — structured records for code blocks, headings, and
//!   outbound links. Every element is serialised as JSON so new kinds
//!   can be added without a schema bump.

use std::path::Path;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use tracing::debug;

use crate::extractor::{ext_of, Extractor};
use crate::types::{ExtractError, ExtractResult, ExtractedDoc, PageText};

/// Markdown extractor. Handles `.md`, `.markdown`, `.mdown`, `.mkd`.
#[derive(Debug, Default, Clone, Copy)]
pub struct MarkdownExtractor;

impl Extractor for MarkdownExtractor {
    fn kinds(&self) -> &[&'static str] {
        &["md", "markdown", "mdown", "mkd"]
    }

    fn extract(&self, path: &Path) -> ExtractResult<ExtractedDoc> {
        let ext = ext_of(path);
        if !self.kinds().contains(&ext.as_str()) {
            return Err(ExtractError::Unsupported {
                path: path.to_path_buf(),
                kind: ext,
            });
        }
        let body = std::fs::read_to_string(path).map_err(|source| ExtractError::Io {
            path: path.to_path_buf(),
            source,
        })?;

        let mut doc = ExtractedDoc::empty("markdown", path);

        // Walk the pulldown stream, track current section, code blocks,
        // and links. We keep state machines simple — nesting of headings
        // inside other block tags is invalid Markdown, so a flat model
        // survives every real-world file.
        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_TABLES);
        opts.insert(Options::ENABLE_FOOTNOTES);
        opts.insert(Options::ENABLE_STRIKETHROUGH);
        opts.insert(Options::ENABLE_TASKLISTS);
        opts.insert(Options::ENABLE_SMART_PUNCTUATION);
        let parser = Parser::new_ext(&body, opts);

        let mut section_index: u32 = 0;
        let mut current_heading: Option<String> = None;
        let mut current_body = String::new();
        let mut heading_buf: Option<String> = None;
        let mut in_code: Option<(String, String)> = None; // (lang, accumulated text)
        let mut in_link: Option<String> = None; // destination

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    // Push the previous section.
                    if current_heading.is_some() || !current_body.is_empty() {
                        section_index += 1;
                        doc.pages.push(PageText {
                            index: section_index,
                            heading: current_heading.clone(),
                            text: std::mem::take(&mut current_body).trim().to_string(),
                        });
                    }
                    heading_buf = Some(String::new());
                    // Only record h1/h2 as structured heading elements so
                    // TOCs stay trim; deeper headings still flush sections.
                    let _ = level;
                }
                Event::End(TagEnd::Heading(level)) => {
                    let h = heading_buf.take().unwrap_or_default();
                    current_heading = Some(h.clone());
                    doc.elements.push(serde_json::json!({
                        "kind": "heading",
                        "level": heading_level_to_u32(level),
                        "text": h,
                    }));
                }
                Event::Start(Tag::CodeBlock(kind)) => {
                    let lang = match kind {
                        CodeBlockKind::Fenced(s) => s.to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                    in_code = Some((lang, String::new()));
                }
                Event::End(TagEnd::CodeBlock) => {
                    if let Some((lang, code)) = in_code.take() {
                        doc.elements.push(serde_json::json!({
                            "kind": "code_block",
                            "lang": lang,
                            "text": code,
                        }));
                        current_body.push_str("```\n");
                    }
                }
                Event::Start(Tag::Link { dest_url, .. }) => {
                    in_link = Some(dest_url.to_string());
                }
                Event::End(TagEnd::Link) => {
                    if let Some(dest) = in_link.take() {
                        doc.elements.push(serde_json::json!({
                            "kind": "link",
                            "href": dest,
                        }));
                    }
                }
                Event::Text(t) => {
                    if let Some(ref mut h) = heading_buf {
                        h.push_str(&t);
                    } else if let Some((_, ref mut code)) = in_code {
                        code.push_str(&t);
                        current_body.push_str(&t);
                    } else {
                        current_body.push_str(&t);
                    }
                }
                Event::Code(t) => {
                    current_body.push('`');
                    current_body.push_str(&t);
                    current_body.push('`');
                }
                Event::SoftBreak | Event::HardBreak => current_body.push('\n'),
                Event::End(TagEnd::Paragraph) => current_body.push_str("\n\n"),
                _ => {}
            }
        }

        // Flush the tail section.
        if current_heading.is_some() || !current_body.trim().is_empty() {
            section_index += 1;
            doc.pages.push(PageText {
                index: section_index,
                heading: current_heading,
                text: current_body.trim().to_string(),
            });
        }
        if doc.pages.is_empty() {
            doc.pages.push(PageText {
                index: 1,
                heading: None,
                text: body.trim().to_string(),
            });
        }
        doc.recompute_text_from_pages();
        doc.metadata
            .insert("section_count".into(), doc.pages.len().to_string());
        doc.metadata.insert("byte_size".into(), body.len().to_string());

        debug!(
            path = %path.display(),
            sections = doc.pages.len(),
            elements = doc.elements.len(),
            "markdown extracted"
        );
        Ok(doc)
    }
}

fn heading_level_to_u32(l: HeadingLevel) -> u32 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "\
# Title

Intro paragraph with [a link](https://example.com).

## Section A

Body of A.

```rust
fn main() {}
```

## Section B

Body of B.
";

    #[test]
    fn markdown_extracts_sections_code_and_links() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.md");
        std::fs::write(&path, FIXTURE).unwrap();

        let doc = MarkdownExtractor.extract(&path).expect("extract");
        assert_eq!(doc.kind, "markdown");
        assert!(doc.pages.len() >= 2, "expected >=2 sections, got {}", doc.pages.len());
        assert!(doc.text.contains("Body of A"));
        assert!(doc.text.contains("Body of B"));

        let kinds: Vec<&str> = doc
            .elements
            .iter()
            .filter_map(|e| e.get("kind").and_then(|v| v.as_str()))
            .collect();
        assert!(kinds.contains(&"code_block"), "expected code_block; got {kinds:?}");
        assert!(kinds.contains(&"link"), "expected link; got {kinds:?}");
        assert!(kinds.contains(&"heading"), "expected heading; got {kinds:?}");
    }

    #[test]
    fn markdown_rejects_non_markdown() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.pdf");
        std::fs::write(&path, "not markdown").unwrap();
        let err = MarkdownExtractor.extract(&path).unwrap_err();
        assert!(matches!(err, ExtractError::Unsupported { .. }));
    }
}
