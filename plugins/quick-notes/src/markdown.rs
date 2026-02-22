//! Markdown parser and renderer for Quick Notes.
//!
//! Provides a simple markdown parser that converts markdown text into
//! styled text segments for rendering in the terminal.

use serde::{Deserialize, Serialize};

/// Markdown AST node types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Node {
    Document { children: Vec<Node> },
    Heading { level: u8, text: String },
    Paragraph { text: String },
    Bold { text: String },
    Italic { text: String },
    Code { text: String },
    CodeBlock { lang: Option<String>, code: String },
    Link { text: String, url: String },
    List { ordered: bool, items: Vec<String> },
    TaskList { items: Vec<TaskItem> },
    Blockquote { text: String },
    HorizontalRule,
    Text { text: String },
}

/// Task item with checkbox state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskItem {
    pub checked: bool,
    pub text: String,
}

/// Render style for text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
    pub monospace: bool,
    pub fg_color: u8, // ANSI color code (0-255)
}

impl TextStyle {
    /// Default text style.
    pub fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            monospace: false,
            fg_color: 15, // White
        }
    }

    /// Bold text style.
    pub fn bold() -> Self {
        Self {
            bold: true,
            ..Self::default()
        }
    }

    /// Italic text style.
    pub fn italic() -> Self {
        Self {
            italic: true,
            ..Self::default()
        }
    }

    /// Code/monospace style.
    pub fn code() -> Self {
        Self {
            monospace: true,
            fg_color: 11, // Yellow
            ..Self::default()
        }
    }

    /// Heading style with color based on level.
    pub fn heading(level: u8) -> Self {
        let color = match level {
            1 => 14, // Cyan
            2 => 13, // Magenta
            _ => 12, // Blue
        };
        Self {
            bold: true,
            fg_color: color,
            ..Self::default()
        }
    }
}

/// Rendered text segment with style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledText {
    pub text: String,
    pub style: TextStyle,
}

impl StyledText {
    /// Create a new styled text segment.
    pub fn new(text: String, style: TextStyle) -> Self {
        Self { text, style }
    }

    /// Create plain text.
    pub fn plain(text: String) -> Self {
        Self {
            text,
            style: TextStyle::default(),
        }
    }

    /// Convert to ANSI-styled string.
    pub fn to_ansi(&self) -> String {
        let mut codes = Vec::new();
        
        if self.style.bold {
            codes.push("1".to_string());
        }
        if self.style.italic {
            codes.push("3".to_string());
        }
        codes.push(format!("38;5;{}", self.style.fg_color));

        format!("\x1b[{}m{}\x1b[0m", codes.join(";"), self.text)
    }
}

/// Simple markdown parser.
pub struct MarkdownParser;

impl MarkdownParser {
    /// Parse markdown text into an AST.
    pub fn parse(input: &str) -> Node {
        let mut children = Vec::new();
        let mut in_code_block = false;
        let mut code_block_content = String::new();
        let mut code_block_lang: Option<String> = None;

        for line in input.lines() {
            // Code block handling
            if line.starts_with("```") {
                if in_code_block {
                    children.push(Node::CodeBlock {
                        lang: code_block_lang.take(),
                        code: core::mem::take(&mut code_block_content),
                    });
                    in_code_block = false;
                } else {
                    in_code_block = true;
                    code_block_lang = Some(line[3..].trim().to_string());
                }
                continue;
            }

            if in_code_block {
                if !code_block_content.is_empty() {
                    code_block_content.push('\n');
                }
                code_block_content.push_str(line);
                continue;
            }

            // Heading
            if line.starts_with("### ") {
                children.push(Node::Heading {
                    level: 3,
                    text: line[4..].to_string(),
                });
            } else if line.starts_with("## ") {
                children.push(Node::Heading {
                    level: 2,
                    text: line[3..].to_string(),
                });
            } else if line.starts_with("# ") {
                children.push(Node::Heading {
                    level: 1,
                    text: line[2..].to_string(),
                });
            }
            // Horizontal rule
            else if line == "---" || line == "***" || line == "___" {
                children.push(Node::HorizontalRule);
            }
            // Blockquote
            else if line.starts_with("> ") {
                children.push(Node::Blockquote {
                    text: line[2..].to_string(),
                });
            }
            // Task list (checkbox)
            else if line.starts_with("- [ ] ") {
                children.push(Node::TaskList {
                    items: vec![TaskItem {
                        checked: false,
                        text: line[6..].to_string(),
                    }],
                });
            } else if line.starts_with("- [x] ") || line.starts_with("- [X] ") {
                children.push(Node::TaskList {
                    items: vec![TaskItem {
                        checked: true,
                        text: line[6..].to_string(),
                    }],
                });
            }
            // Unordered list
            else if line.starts_with("- ") || line.starts_with("* ") {
                children.push(Node::List {
                    ordered: false,
                    items: vec![line[2..].to_string()],
                });
            }
            // Ordered list
            else if let Some(pos) = line.find(". ") {
                if pos > 0 && pos < 4 {
                    let num = &line[..pos];
                    if num.chars().all(|c| c.is_ascii_digit()) {
                        children.push(Node::List {
                            ordered: true,
                            items: vec![line[pos + 2..].to_string()],
                        });
                        continue;
                    }
                }
                children.push(Node::Paragraph {
                    text: parse_inline(line),
                });
            }
            // Empty line
            else if line.is_empty() {
                // Skip empty lines
            }
            // Paragraph with inline formatting
            else {
                children.push(Node::Paragraph {
                    text: parse_inline(line),
                });
            }
        }

        Node::Document { children }
    }

    /// Render AST to styled text segments.
    pub fn render(node: &Node) -> Vec<StyledText> {
        let mut output = Vec::new();
        render_node(node, &mut output, TextStyle::default());
        output
    }
}

/// Parse inline markdown formatting (bold, italic, code, links).
/// For simplicity, returns the text as-is with basic formatting.
fn parse_inline(text: &str) -> String {
    // TODO: Implement full inline parsing for **bold**, *italic*, `code`, [links](url)
    text.to_string()
}

/// Render a node to styled text segments.
fn render_node(node: &Node, output: &mut Vec<StyledText>, base_style: TextStyle) {
    match node {
        Node::Document { children } => {
            for child in children {
                render_node(child, output, base_style);
            }
        }
        Node::Heading { level, text } => {
            output.push(StyledText::new(text.clone(), TextStyle::heading(*level)));
            output.push(StyledText::plain("\n".to_string()));
        }
        Node::Paragraph { text } => {
            output.push(StyledText::new(text.clone(), base_style));
            output.push(StyledText::plain("\n".to_string()));
        }
        Node::Bold { text } => {
            let style = TextStyle {
                bold: true,
                ..base_style
            };
            output.push(StyledText::new(text.clone(), style));
        }
        Node::Italic { text } => {
            let style = TextStyle {
                italic: true,
                ..base_style
            };
            output.push(StyledText::new(text.clone(), style));
        }
        Node::Code { text } => {
            output.push(StyledText::new(format!("`{}`", text), TextStyle::code()));
        }
        Node::CodeBlock { lang, code } => {
            if let Some(l) = lang {
                if !l.is_empty() {
                    output.push(StyledText::new(
                        format!("```{}\n", l),
                        TextStyle {
                            monospace: true,
                            fg_color: 8,
                            ..base_style
                        },
                    ));
                } else {
                    output.push(StyledText::new(
                        "```\n".to_string(),
                        TextStyle {
                            monospace: true,
                            fg_color: 8,
                            ..base_style
                        },
                    ));
                }
            } else {
                output.push(StyledText::new(
                    "```\n".to_string(),
                    TextStyle {
                        monospace: true,
                        fg_color: 8,
                        ..base_style
                    },
                ));
            }
            output.push(StyledText::new(
                code.clone(),
                TextStyle {
                    monospace: true,
                    fg_color: 15,
                    ..base_style
                },
            ));
            output.push(StyledText::new(
                "\n```\n".to_string(),
                TextStyle {
                    monospace: true,
                    fg_color: 8,
                    ..base_style
                },
            ));
        }
        Node::Link { text, url: _ } => {
            let style = TextStyle {
                fg_color: 4, // Blue
                ..base_style
            };
            output.push(StyledText::new(text.clone(), style));
        }
        Node::List { ordered, items } => {
            for (i, item) in items.iter().enumerate() {
                let prefix = if *ordered {
                    format!("{}. ", i + 1)
                } else {
                    "• ".to_string()
                };
                output.push(StyledText::new(prefix, base_style));
                output.push(StyledText::new(item.clone(), base_style));
                output.push(StyledText::plain("\n".to_string()));
            }
        }
        Node::TaskList { items } => {
            for item in items {
                let checkbox = if item.checked { "☑ " } else { "☐ " };
                let style = if item.checked {
                    TextStyle {
                        fg_color: 2, // Green for completed
                        ..base_style
                    }
                } else {
                    base_style
                };
                output.push(StyledText::new(checkbox.to_string(), style));
                output.push(StyledText::new(item.text.clone(), style));
                output.push(StyledText::plain("\n".to_string()));
            }
        }
        Node::Blockquote { text } => {
            output.push(StyledText::new(
                "│ ".to_string(),
                TextStyle {
                    fg_color: 8,
                    ..base_style
                },
            ));
            output.push(StyledText::new(
                format!("{}\n", text),
                TextStyle {
                    italic: true,
                    fg_color: 7,
                    ..base_style
                },
            ));
        }
        Node::HorizontalRule => {
            output.push(StyledText::new(
                "─".repeat(40) + "\n",
                TextStyle {
                    fg_color: 8,
                    ..base_style
                },
            ));
        }
        Node::Text { text } => {
            output.push(StyledText::new(text.clone(), base_style));
        }
    }
}

/// Render markdown text to ANSI-styled string.
pub fn render_to_ansi(markdown: &str) -> String {
    let ast = MarkdownParser::parse(markdown);
    let styled = MarkdownParser::render(&ast);
    styled.iter().map(|s| s.to_ansi()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_heading() {
        let md = "# Hello World";
        let ast = MarkdownParser::parse(md);
        
        match ast {
            Node::Document { children } => {
                assert_eq!(children.len(), 1);
                match &children[0] {
                    Node::Heading { level, text } => {
                        assert_eq!(*level, 1);
                        assert_eq!(text, "Hello World");
                    }
                    _ => panic!("Expected heading node"),
                }
            }
            _ => panic!("Expected document node"),
        }
    }

    #[test]
    fn test_parse_task_list() {
        let md = "- [ ] Task 1\n- [x] Task 2";
        let ast = MarkdownParser::parse(md);
        
        match ast {
            Node::Document { children } => {
                assert_eq!(children.len(), 2);
                
                match &children[0] {
                    Node::TaskList { items } => {
                        assert_eq!(items.len(), 1);
                        assert!(!items[0].checked);
                        assert_eq!(items[0].text, "Task 1");
                    }
                    _ => panic!("Expected task list node"),
                }
                
                match &children[1] {
                    Node::TaskList { items } => {
                        assert_eq!(items.len(), 1);
                        assert!(items[0].checked);
                        assert_eq!(items[0].text, "Task 2");
                    }
                    _ => panic!("Expected task list node"),
                }
            }
            _ => panic!("Expected document node"),
        }
    }

    #[test]
    fn test_render_heading() {
        let ast = Node::Heading {
            level: 1,
            text: "Title".to_string(),
        };
        
        let styled = MarkdownParser::render(&ast);
        assert_eq!(styled.len(), 2);
        assert!(styled[0].style.bold);
        assert_eq!(styled[0].style.fg_color, 14); // Cyan
        assert_eq!(styled[0].text, "Title");
    }

    #[test]
    fn test_render_task_list() {
        let ast = Node::TaskList {
            items: vec![
                TaskItem {
                    checked: false,
                    text: "Todo".to_string(),
                },
                TaskItem {
                    checked: true,
                    text: "Done".to_string(),
                },
            ],
        };
        
        let styled = MarkdownParser::render(&ast);
        // Each task produces 3 segments: checkbox, text, newline
        assert_eq!(styled.len(), 6);
        
        // Unchecked task
        assert_eq!(styled[0].text, "☐ ");
        assert_eq!(styled[0].style.fg_color, 15); // Default
        
        // Checked task
        assert_eq!(styled[3].text, "☑ ");
        assert_eq!(styled[3].style.fg_color, 2); // Green
    }

    #[test]
    fn test_ansi_output() {
        let styled = StyledText::new("Hello".to_string(), TextStyle::bold());
        let ansi = styled.to_ansi();
        
        assert!(ansi.contains("\x1b["));
        assert!(ansi.contains("1")); // Bold code
        assert!(ansi.contains("Hello"));
        assert!(ansi.contains("\x1b[0m")); // Reset
    }

    #[test]
    fn test_full_render() {
        let md = "# Title\n\nParagraph\n\n- [ ] Task";
        let output = render_to_ansi(md);
        
        assert!(output.contains("Title"));
        assert!(output.contains("Paragraph"));
        assert!(output.contains("☐"));
    }
}
