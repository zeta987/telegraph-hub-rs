use crate::telegraph::types::Node;

/// Tags allowed by the Telegraph API specification.
const ALLOWED_TAGS: &[&str] = &[
    "a",
    "aside",
    "b",
    "blockquote",
    "br",
    "code",
    "em",
    "figcaption",
    "figure",
    "h3",
    "h4",
    "hr",
    "i",
    "iframe",
    "img",
    "li",
    "ol",
    "p",
    "pre",
    "s",
    "strong",
    "u",
    "ul",
    "video",
];

/// Void elements that must not have a closing tag.
const VOID_ELEMENTS: &[&str] = &["br", "hr", "img"];

/// Attributes allowed through the whitelist.
const ALLOWED_ATTRS: &[&str] = &["href", "src"];

/// Escape special HTML characters to prevent XSS.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

/// Convert a slice of Telegraph `Node` values into a safe HTML string.
///
/// Only whitelisted tags and attributes are rendered. Unknown tags
/// are replaced with `<div>`. All text content is HTML-escaped.
/// `<iframe>` elements receive a `sandbox` attribute for security.
pub fn render_nodes_to_html(nodes: &[Node]) -> String {
    let mut out = String::new();
    for node in nodes {
        render_node(node, &mut out);
    }
    out
}

fn render_node(node: &Node, out: &mut String) {
    match node {
        Node::Text(text) => out.push_str(&escape_html(text)),
        Node::Element(el) => {
            let tag = if ALLOWED_TAGS.contains(&el.tag.as_str()) {
                el.tag.as_str()
            } else {
                "div"
            };

            out.push('<');
            out.push_str(tag);

            // Render whitelisted attributes only
            if let Some(attrs) = &el.attrs {
                for key in ALLOWED_ATTRS {
                    if let Some(value) = attrs.get(*key) {
                        out.push(' ');
                        out.push_str(key);
                        out.push_str("=\"");
                        out.push_str(&escape_html(value));
                        out.push('"');
                    }
                }
            }

            // Sandbox iframes for security
            if tag == "iframe" {
                out.push_str(" sandbox");
            }

            if VOID_ELEMENTS.contains(&tag) {
                out.push_str(" />");
            } else {
                out.push('>');
                if let Some(children) = &el.children {
                    for child in children {
                        render_node(child, out);
                    }
                }
                out.push_str("</");
                out.push_str(tag);
                out.push('>');
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telegraph::types::NodeElement;
    use std::collections::HashMap;

    #[test]
    fn render_paragraph_text() {
        let nodes = vec![Node::Element(NodeElement {
            tag: "p".to_string(),
            attrs: None,
            children: Some(vec![Node::Text("Hello world".to_string())]),
        })];
        assert_eq!(render_nodes_to_html(&nodes), "<p>Hello world</p>");
    }

    #[test]
    fn render_link_with_href() {
        let mut attrs = HashMap::new();
        attrs.insert("href".to_string(), "https://example.com".to_string());
        let nodes = vec![Node::Element(NodeElement {
            tag: "a".to_string(),
            attrs: Some(attrs),
            children: Some(vec![Node::Text("click".to_string())]),
        })];
        assert_eq!(
            render_nodes_to_html(&nodes),
            "<a href=\"https://example.com\">click</a>"
        );
    }

    #[test]
    fn render_nested_elements() {
        let nodes = vec![Node::Element(NodeElement {
            tag: "p".to_string(),
            attrs: None,
            children: Some(vec![
                Node::Text("Hello ".to_string()),
                Node::Element(NodeElement {
                    tag: "strong".to_string(),
                    attrs: None,
                    children: Some(vec![Node::Text("world".to_string())]),
                }),
            ]),
        })];
        assert_eq!(
            render_nodes_to_html(&nodes),
            "<p>Hello <strong>world</strong></p>"
        );
    }

    #[test]
    fn escape_xss_in_text() {
        let nodes = vec![Node::Text("<script>alert('xss')</script>".to_string())];
        assert_eq!(
            render_nodes_to_html(&nodes),
            "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
        );
    }

    #[test]
    fn escape_xss_in_attribute() {
        let mut attrs = HashMap::new();
        attrs.insert("href".to_string(), "javascript:alert(\"xss\")".to_string());
        let nodes = vec![Node::Element(NodeElement {
            tag: "a".to_string(),
            attrs: Some(attrs),
            children: Some(vec![Node::Text("click".to_string())]),
        })];
        assert_eq!(
            render_nodes_to_html(&nodes),
            "<a href=\"javascript:alert(&quot;xss&quot;)\">click</a>"
        );
    }

    #[test]
    fn unknown_tag_becomes_div() {
        let nodes = vec![Node::Element(NodeElement {
            tag: "script".to_string(),
            attrs: None,
            children: Some(vec![Node::Text("evil()".to_string())]),
        })];
        assert_eq!(render_nodes_to_html(&nodes), "<div>evil()</div>");
    }

    #[test]
    fn void_elements_self_close() {
        let nodes = vec![
            Node::Element(NodeElement {
                tag: "br".to_string(),
                attrs: None,
                children: None,
            }),
            Node::Element(NodeElement {
                tag: "hr".to_string(),
                attrs: None,
                children: None,
            }),
        ];
        assert_eq!(render_nodes_to_html(&nodes), "<br /><hr />");
    }

    #[test]
    fn img_with_src() {
        let mut attrs = HashMap::new();
        attrs.insert("src".to_string(), "https://example.com/img.jpg".to_string());
        let nodes = vec![Node::Element(NodeElement {
            tag: "img".to_string(),
            attrs: Some(attrs),
            children: None,
        })];
        assert_eq!(
            render_nodes_to_html(&nodes),
            "<img src=\"https://example.com/img.jpg\" />"
        );
    }

    #[test]
    fn iframe_gets_sandbox() {
        let mut attrs = HashMap::new();
        attrs.insert(
            "src".to_string(),
            "https://www.youtube.com/embed/123".to_string(),
        );
        let nodes = vec![Node::Element(NodeElement {
            tag: "iframe".to_string(),
            attrs: Some(attrs),
            children: None,
        })];
        assert_eq!(
            render_nodes_to_html(&nodes),
            "<iframe src=\"https://www.youtube.com/embed/123\" sandbox></iframe>"
        );
    }

    #[test]
    fn disallowed_attributes_stripped() {
        let mut attrs = HashMap::new();
        attrs.insert("onclick".to_string(), "alert(1)".to_string());
        attrs.insert("href".to_string(), "https://safe.com".to_string());
        let nodes = vec![Node::Element(NodeElement {
            tag: "a".to_string(),
            attrs: Some(attrs),
            children: Some(vec![Node::Text("link".to_string())]),
        })];
        // Only href should be rendered, onclick stripped
        assert_eq!(
            render_nodes_to_html(&nodes),
            "<a href=\"https://safe.com\">link</a>"
        );
    }

    #[test]
    fn empty_nodes() {
        let nodes: Vec<Node> = vec![];
        assert_eq!(render_nodes_to_html(&nodes), "");
    }
}
