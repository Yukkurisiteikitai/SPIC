use crate::model::{Block, BlockKind, Presentation, Slide};

pub const REVEAL_MARKER: &str = "<!-- slidecli:next -->";
pub const REVEAL_START_MARKER: &str = "<!-- slidecli:next:start -->";
pub const REVEAL_END_MARKER: &str = "<!-- slidecli:next:end -->";

#[derive(Debug, Clone, PartialEq)]
enum PendingBlockMeta {
    Exec {
        lang: Option<String>,
        signature: Option<String>,
    },
    Output,
}

pub fn serialize(presentation: &Presentation) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "<!-- slidecli:presentation font=\"{}\" font-size={} -->\n\n",
        escape_attr(&presentation.font_name),
        presentation.font_size
    ));

    for (slide_idx, slide) in presentation.slides.iter().enumerate() {
        if slide_idx > 0 {
            out.push_str("\n---\n\n");
        }

        out.push_str(&format!(
            "<!-- slidecli:slide title=\"{}\" -->\n\n",
            escape_attr(&effective_slide_title(slide, slide_idx))
        ));

        for (block_idx, block) in slide.blocks.iter().enumerate() {
            if block_idx > 0 {
                out.push('\n');
            }

            match &block.kind {
                BlockKind::Heading { level } => {
                    let hashes = "#".repeat((*level).clamp(1, 3) as usize);
                    out.push_str(&format!("{} {}\n", hashes, block.content));
                }
                BlockKind::Text => {
                    out.push_str(&block.content);
                    out.push('\n');
                }
                BlockKind::Code { lang } => {
                    out.push_str(&format!("```{}\n", lang));
                    out.push_str(&block.content);
                    if !block.content.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str("```\n");
                }
                BlockKind::Exec { lang, signature } => {
                    out.push_str(&format!(
                        "<!-- slidecli:block type=exec lang=\"{}\"{} -->\n",
                        escape_attr(lang),
                        signature
                            .as_ref()
                            .map(|sig| format!(" sig=\"{}\"", escape_attr(sig)))
                            .unwrap_or_default()
                    ));
                    out.push_str(&format!("```{}\n", lang));
                    out.push_str(&block.content);
                    if !block.content.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str("```\n");
                }
                BlockKind::OutputPlaceholder => {
                    out.push_str("<!-- slidecli:block type=output -->\n");
                    if !block.content.is_empty() {
                        out.push_str("```text\n");
                        out.push_str(&block.content);
                        if !block.content.ends_with('\n') {
                            out.push('\n');
                        }
                        out.push_str("```\n");
                    }
                }
                BlockKind::Separator => {
                    out.push_str("<!-- slidecli:block type=separator -->\n");
                }
            }
        }
    }

    out
}

pub fn deserialize(src: &str) -> Result<Presentation, String> {
    let normalized = src.replace("\r\n", "\n");
    let slide_sections = split_slides(&normalized);

    let mut presentation = Presentation::blank();
    let mut slides = Vec::new();

    for (slide_idx, section) in slide_sections.iter().enumerate() {
        let parsed = parse_slide(section, slide_idx)?;
        if !parsed.blocks.is_empty() || !section.trim().is_empty() || slide_sections.len() == 1 {
            slides.push(parsed);
        }
    }

    if slides.is_empty() {
        slides.push(Slide::new("Slide 1"));
    }
    renumber_blocks(&mut slides);
    presentation.slides = slides;

    if let Some((font_name, font_size)) = parse_presentation_meta(&normalized) {
        presentation.font_name = font_name;
        presentation.font_size = font_size;
    }

    Ok(presentation)
}

pub fn reveal_marker_count(content: &str) -> usize {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed == REVEAL_MARKER || trimmed == REVEAL_START_MARKER
        })
        .count()
}

pub fn visible_reveal_content(content: &str, reveal_step: usize) -> String {
    let mut seen_markers = 0usize;
    let mut in_hidden_region = false;
    let mut visible = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == REVEAL_MARKER {
            seen_markers += 1;
            if seen_markers > reveal_step {
                break;
            }
            continue;
        }

        if trimmed == REVEAL_START_MARKER {
            seen_markers += 1;
            in_hidden_region = seen_markers > reveal_step;
            continue;
        }

        if trimmed == REVEAL_END_MARKER {
            in_hidden_region = false;
            continue;
        }

        if in_hidden_region {
            continue;
        }

        visible.push(line);
    }

    visible.join("\n")
}

fn parse_presentation_meta(src: &str) -> Option<(String, u8)> {
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (directive, attrs) = parse_comment_directive(trimmed)?;
        if directive == "slidecli:presentation" {
            let font_name = attrs
                .iter()
                .find(|(key, _)| *key == "font")
                .map(|(_, value)| value.clone())
                .unwrap_or_else(|| "JetBrains Mono".to_string());
            let font_size = attrs
                .iter()
                .find(|(key, _)| *key == "font-size")
                .and_then(|(_, value)| value.parse::<u8>().ok())
                .unwrap_or(14);
            return Some((font_name, font_size));
        }
        break;
    }
    None
}

fn split_slides(src: &str) -> Vec<String> {
    let mut sections = Vec::new();
    let mut current = Vec::new();
    let mut in_fence = false;

    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
        }

        if !in_fence && trimmed == "---" {
            sections.push(current.join("\n"));
            current.clear();
            continue;
        }

        current.push(line);
    }

    sections.push(current.join("\n"));
    sections
}

fn parse_slide(section: &str, slide_idx: usize) -> Result<Slide, String> {
    let lines: Vec<&str> = section.lines().collect();
    let mut blocks = Vec::new();
    let mut text_lines = Vec::new();
    let mut pending_block: Option<PendingBlockMeta> = None;
    let mut explicit_title: Option<String> = None;
    let mut next_id = 1u64;
    let mut idx = 0usize;

    while idx < lines.len() {
        let line = lines[idx];
        let trimmed = line.trim();

        if trimmed.is_empty() {
            flush_text_block(&mut blocks, &mut text_lines, &mut next_id);
            idx += 1;
            continue;
        }

        if let Some((directive, attrs)) = parse_comment_directive(trimmed) {
            if matches!(
                directive.as_str(),
                "slidecli:next" | "slidecli:next:start" | "slidecli:next:end"
            ) {
                flush_pending_block(&mut blocks, &mut pending_block, &mut next_id);
                text_lines.push(line.to_string());
                idx += 1;
                continue;
            }

            flush_text_block(&mut blocks, &mut text_lines, &mut next_id);

            match directive.as_str() {
                "slidecli:presentation" => {}
                "slidecli:slide" => {
                    explicit_title = attrs
                        .iter()
                        .find(|(key, _)| *key == "title")
                        .map(|(_, value)| value.clone());
                }
                "slidecli:block" => {
                    let block_type = attrs
                        .iter()
                        .find(|(key, _)| *key == "type")
                        .map(|(_, value)| value.as_str());
                    match block_type {
                        Some("exec") => {
                            let lang = attrs
                                .iter()
                                .find(|(key, _)| *key == "lang")
                                .map(|(_, value)| value.clone());
                            let signature = attrs
                                .iter()
                                .find(|(key, _)| *key == "sig")
                                .map(|(_, value)| value.clone());
                            pending_block = Some(PendingBlockMeta::Exec { lang, signature });
                        }
                        Some("output") => {
                            pending_block = Some(PendingBlockMeta::Output);
                        }
                        Some("separator") => {
                            blocks.push(Block::new(
                                next_id,
                                BlockKind::Separator,
                                String::new(),
                            ));
                            next_id += 1;
                            pending_block = None;
                        }
                        _ => {}
                    }
                }
                _ => {
                    text_lines.push(line.to_string());
                }
            }
            idx += 1;
            continue;
        }

        if trimmed.starts_with("```") {
            flush_text_block(&mut blocks, &mut text_lines, &mut next_id);

            let fence_info = trimmed.trim_start_matches("```").trim();
            let mut code_lines = Vec::new();
            idx += 1;
            while idx < lines.len() && lines[idx].trim() != "```" {
                code_lines.push(lines[idx]);
                idx += 1;
            }
            if idx == lines.len() {
                return Err("コードブロックの終端 ``` がありません".to_string());
            }

            let content = code_lines.join("\n");
            let (fence_lang, fence_exec, fence_sig) = parse_fence_info(fence_info);
            let block = match pending_block.take() {
                Some(PendingBlockMeta::Exec { lang, signature }) => Block::new(
                    next_id,
                    BlockKind::Exec {
                        lang: lang
                            .or(fence_lang)
                            .unwrap_or_else(|| "text".to_string()),
                        signature: signature.or(fence_sig),
                    },
                    content,
                ),
                Some(PendingBlockMeta::Output) => Block::new(
                    next_id,
                    BlockKind::OutputPlaceholder,
                    content,
                ),
                None if fence_exec => Block::new(
                    next_id,
                    BlockKind::Exec {
                        lang: fence_lang.unwrap_or_else(|| "text".to_string()),
                        signature: fence_sig,
                    },
                    content,
                ),
                None => Block::new(
                    next_id,
                    BlockKind::Code {
                        lang: fence_lang.unwrap_or_else(|| "text".to_string()),
                    },
                    content,
                ),
            };
            blocks.push(block);
            next_id += 1;
            idx += 1;
            continue;
        }

        if let Some((level, title)) = parse_heading(trimmed) {
            flush_pending_block(&mut blocks, &mut pending_block, &mut next_id);
            flush_text_block(&mut blocks, &mut text_lines, &mut next_id);
            blocks.push(Block::new(next_id, BlockKind::Heading { level }, title));
            next_id += 1;
            idx += 1;
            continue;
        }

        flush_pending_block(&mut blocks, &mut pending_block, &mut next_id);
        text_lines.push(line.to_string());
        idx += 1;
    }

    flush_text_block(&mut blocks, &mut text_lines, &mut next_id);
    flush_pending_block(&mut blocks, &mut pending_block, &mut next_id);

    let title = explicit_title.unwrap_or_else(|| {
        blocks
            .iter()
            .find_map(|block| match &block.kind {
                BlockKind::Heading { .. } => Some(block.content.clone()),
                _ => None,
            })
            .unwrap_or_else(|| format!("Slide {}", slide_idx + 1))
    });

    Ok(Slide { title, blocks })
}

fn flush_text_block(blocks: &mut Vec<Block>, text_lines: &mut Vec<String>, next_id: &mut u64) {
    if text_lines.is_empty() {
        return;
    }

    blocks.push(Block::new(
        *next_id,
        BlockKind::Text,
        text_lines.join("\n"),
    ));
    *next_id += 1;
    text_lines.clear();
}

fn flush_pending_block(
    blocks: &mut Vec<Block>,
    pending_block: &mut Option<PendingBlockMeta>,
    next_id: &mut u64,
) {
    let Some(meta) = pending_block.take() else {
        return;
    };

    match meta {
        PendingBlockMeta::Exec { lang, signature } => blocks.push(Block::new(
            *next_id,
            BlockKind::Exec {
                lang: lang.unwrap_or_else(|| "text".to_string()),
                signature,
            },
            String::new(),
        )),
        PendingBlockMeta::Output => {
            blocks.push(Block::new(*next_id, BlockKind::OutputPlaceholder, String::new()))
        }
    }
    *next_id += 1;
}

fn parse_heading(line: &str) -> Option<(u8, String)> {
    let mut count = 0usize;
    for ch in line.chars() {
        if ch == '#' {
            count += 1;
        } else {
            break;
        }
    }
    if !(1..=3).contains(&count) {
        return None;
    }

    let rest = line[count..].trim_start();
    if rest.is_empty() {
        return None;
    }

    Some((count as u8, rest.to_string()))
}

fn parse_fence_info(info: &str) -> (Option<String>, bool, Option<String>) {
    let mut lang = None;
    let mut is_exec = false;
    let mut signature = None;

    for token in info.split_whitespace() {
        if token == "exec" {
            is_exec = true;
        } else if let Some(value) = token.strip_prefix("sig=") {
            signature = Some(value.trim_matches('"').to_string());
        } else if token.starts_with("sig:") {
            signature = Some(token.to_string());
        } else if lang.is_none() {
            lang = Some(token.to_string());
        }
    }

    (lang, is_exec, signature)
}

fn parse_comment_directive(line: &str) -> Option<(String, Vec<(String, String)>)> {
    let inner = line
        .strip_prefix("<!--")?
        .strip_suffix("-->")?
        .trim();
    let mut chars = inner.chars().peekable();
    let mut directive = String::new();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            break;
        }
        directive.push(ch);
        chars.next();
    }

    if !directive.starts_with("slidecli:") {
        return None;
    }

    while chars.peek().is_some_and(|ch| ch.is_whitespace()) {
        chars.next();
    }

    let rest: String = chars.collect();
    Some((directive, parse_attrs(&rest)))
}

fn parse_attrs(input: &str) -> Vec<(String, String)> {
    let bytes = input.as_bytes();
    let mut idx = 0usize;
    let mut attrs = Vec::new();

    while idx < bytes.len() {
        while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        if idx >= bytes.len() {
            break;
        }

        let key_start = idx;
        while idx < bytes.len() && !bytes[idx].is_ascii_whitespace() && bytes[idx] != b'=' {
            idx += 1;
        }
        let key = input[key_start..idx].to_string();

        while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }

        if idx >= bytes.len() || bytes[idx] != b'=' {
            attrs.push((key, String::new()));
            continue;
        }
        idx += 1;

        while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }

        let value = if idx < bytes.len() && bytes[idx] == b'"' {
            idx += 1;
            let mut value = String::new();
            while idx < bytes.len() {
                let ch = bytes[idx] as char;
                idx += 1;
                match ch {
                    '\\' if idx < bytes.len() => {
                        value.push(bytes[idx] as char);
                        idx += 1;
                    }
                    '"' => break,
                    _ => value.push(ch),
                }
            }
            value
        } else {
            let value_start = idx;
            while idx < bytes.len() && !bytes[idx].is_ascii_whitespace() {
                idx += 1;
            }
            input[value_start..idx].to_string()
        };

        attrs.push((key, value));
    }

    attrs
}

fn escape_attr(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn effective_slide_title(slide: &Slide, slide_idx: usize) -> String {
    slide
        .blocks
        .iter()
        .find_map(|block| match block.kind {
            BlockKind::Heading { .. } => Some(block.content.clone()),
            _ => None,
        })
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| {
            if slide.title.trim().is_empty() {
                format!("Slide {}", slide_idx + 1)
            } else {
                slide.title.clone()
            }
        })
}

fn renumber_blocks(slides: &mut [Slide]) {
    let mut next_id = 1u64;
    for slide in slides {
        for block in &mut slide.blocks {
            block.id = next_id;
            next_id += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        deserialize, reveal_marker_count, serialize, visible_reveal_content, REVEAL_END_MARKER,
        REVEAL_MARKER, REVEAL_START_MARKER,
    };
    use crate::model::Presentation;

    #[test]
    fn round_trips_demo_presentation() {
        let presentation = Presentation::demo();
        let markdown = serialize(&presentation);
        let reparsed = deserialize(&markdown).unwrap();
        let markdown_again = serialize(&reparsed);
        assert_eq!(markdown_again, markdown);
    }

    #[test]
    fn parses_existing_markdown_with_exec_comments() {
        let src = r#"<!-- slidecli:presentation font="Fira Code" font-size=20 -->

<!-- slidecli:slide title="Demo" -->

# Demo

Intro text

<!-- slidecli:block type=exec lang="bash" sig="sig:ed25519:abc123" -->
```bash
echo hi
```

<!-- slidecli:block type=output -->
```text
hi
```
"#;

        let presentation = deserialize(src).unwrap();
        assert_eq!(presentation.font_name, "Fira Code");
        assert_eq!(presentation.font_size, 20);
        assert_eq!(presentation.slides.len(), 1);
        assert_eq!(presentation.slides[0].title, "Demo");
        assert_eq!(presentation.slides[0].blocks.len(), 4);
    }

    #[test]
    fn keeps_reveal_markers_inside_text_blocks() {
        let src = "# Intro\n\nline 1\n<!-- slidecli:next:start -->\nline 2\nline 3\n<!-- slidecli:next:end -->\nline 4\n";
        let presentation = deserialize(src).unwrap();
        let text = &presentation.slides[0].blocks[1].content;
        assert!(text.contains(REVEAL_START_MARKER));
        assert!(text.contains(REVEAL_END_MARKER));
        assert_eq!(reveal_marker_count(text), 1);
        assert_eq!(visible_reveal_content(text, 0), "line 1\nline 4");
        assert_eq!(visible_reveal_content(text, 1), "line 1\nline 2\nline 3\nline 4");
    }

    #[test]
    fn keeps_legacy_reveal_marker_working() {
        let src = "# Intro\n\nline 1\n<!-- slidecli:next -->\nline 2\n";
        let presentation = deserialize(src).unwrap();
        let text = &presentation.slides[0].blocks[1].content;
        assert!(text.contains(REVEAL_MARKER));
        assert_eq!(reveal_marker_count(text), 1);
        assert_eq!(visible_reveal_content(text, 0), "line 1");
        assert_eq!(visible_reveal_content(text, 1), "line 1\nline 2");
    }
}
