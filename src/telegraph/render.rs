use url::Url;

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
///
/// `base` is the Telegraph page URL used to resolve relative `href`/`src`
/// attributes. When `None`, relative URLs are dropped (not resolved against
/// the app origin), while absolute URLs are still validated against the
/// scheme allowlist.
pub fn render_nodes_to_html(nodes: &[Node], base: Option<&Url>) -> String {
    let mut out = String::new();
    for node in nodes {
        render_node(node, &mut out, base);
    }
    out
}

fn render_node(node: &Node, out: &mut String, base: Option<&Url>) {
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

            // Render whitelisted attributes only, with URL scheme validation
            if let Some(attrs) = &el.attrs {
                for key in ALLOWED_ATTRS {
                    if let Some(value) = attrs.get(*key)
                        && let Some(clean) = sanitize_url_attr(tag, key, value, base)
                    {
                        out.push(' ');
                        out.push_str(key);
                        out.push_str("=\"");
                        out.push_str(&escape_html(&clean));
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
                        render_node(child, out, base);
                    }
                }
                out.push_str("</");
                out.push_str(tag);
                out.push('>');
            }
        }
    }
}

/// Validate a `href`/`src` attribute value against a tag-aware scheme allowlist.
///
/// Returns the cleaned URL string when validation passes, or `None` when the
/// attribute should be dropped entirely. Relative and protocol-relative URLs
/// are resolved against `base`; when `base` is `None`, relative inputs are
/// rejected.
///
/// The scheme comparison is case-insensitive (the `url` crate normalizes the
/// scheme to lowercase) and tolerant of leading/trailing whitespace.
///
/// - `a[href]`: `http`, `https`, `mailto`, `tel`
/// - `img[src]`, `video[src]`, `iframe[src]`: `https` only
fn sanitize_url_attr(tag: &str, attr: &str, raw: &str, base: Option<&Url>) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let allowed: &[&str] = match (tag, attr) {
        ("a", "href") => &["http", "https", "mailto", "tel"],
        ("img", "src") | ("video", "src") | ("iframe", "src") => &["https"],
        _ => return None,
    };

    match Url::parse(trimmed) {
        Ok(parsed) => {
            if !allowed.contains(&parsed.scheme()) {
                return None;
            }
            // Preserve the trimmed original for absolute URLs to avoid trailing-slash
            // and similar normalization differences. The HTML escape on the caller side
            // is the second line of defense against quote-breaking.
            Some(trimmed.to_string())
        }
        Err(_) => {
            // Relative or protocol-relative URL — needs a base to resolve.
            let base = base?;
            let joined = base.join(trimmed).ok()?;
            if !allowed.contains(&joined.scheme()) {
                return None;
            }
            Some(joined.to_string())
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
        assert_eq!(render_nodes_to_html(&nodes, None), "<p>Hello world</p>");
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
            render_nodes_to_html(&nodes, None),
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
            render_nodes_to_html(&nodes, None),
            "<p>Hello <strong>world</strong></p>"
        );
    }

    #[test]
    fn escape_xss_in_text() {
        let nodes = vec![Node::Text("<script>alert('xss')</script>".to_string())];
        assert_eq!(
            render_nodes_to_html(&nodes, None),
            "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
        );
    }

    #[test]
    fn javascript_href_is_dropped() {
        // Previously asserted that the javascript: scheme was merely
        // HTML-escaped and still emitted as a live href — that encoded the XSS
        // bug as expected behavior. The renderer now drops the attribute.
        let mut attrs = HashMap::new();
        attrs.insert("href".to_string(), "javascript:alert(\"xss\")".to_string());
        let nodes = vec![Node::Element(NodeElement {
            tag: "a".to_string(),
            attrs: Some(attrs),
            children: Some(vec![Node::Text("click".to_string())]),
        })];
        let rendered = render_nodes_to_html(&nodes, None);
        assert_eq!(rendered, "<a>click</a>");
        assert!(!rendered.contains("href="));
    }

    #[test]
    fn unknown_tag_becomes_div() {
        let nodes = vec![Node::Element(NodeElement {
            tag: "script".to_string(),
            attrs: None,
            children: Some(vec![Node::Text("evil()".to_string())]),
        })];
        assert_eq!(render_nodes_to_html(&nodes, None), "<div>evil()</div>");
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
        assert_eq!(render_nodes_to_html(&nodes, None), "<br /><hr />");
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
            render_nodes_to_html(&nodes, None),
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
            render_nodes_to_html(&nodes, None),
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
            render_nodes_to_html(&nodes, None),
            "<a href=\"https://safe.com\">link</a>"
        );
    }

    #[test]
    fn empty_nodes() {
        let nodes: Vec<Node> = vec![];
        assert_eq!(render_nodes_to_html(&nodes, None), "");
    }

    // ---------- regression tests for harden-preview-url-validation ----------

    fn telegraph_base() -> Url {
        Url::parse("https://telegra.ph/Post-01-01").unwrap()
    }

    fn anchor(href: &str) -> Vec<Node> {
        let mut attrs = HashMap::new();
        attrs.insert("href".to_string(), href.to_string());
        vec![Node::Element(NodeElement {
            tag: "a".to_string(),
            attrs: Some(attrs),
            children: Some(vec![Node::Text("click".to_string())]),
        })]
    }

    fn img(src: &str) -> Vec<Node> {
        let mut attrs = HashMap::new();
        attrs.insert("src".to_string(), src.to_string());
        vec![Node::Element(NodeElement {
            tag: "img".to_string(),
            attrs: Some(attrs),
            children: None,
        })]
    }

    #[test]
    fn lowercase_javascript_href_is_dropped() {
        let nodes = anchor("javascript:alert(1)");
        let rendered = render_nodes_to_html(&nodes, None);
        assert!(!rendered.contains("href="));
        assert_eq!(rendered, "<a>click</a>");
    }

    #[test]
    fn uppercase_javascript_href_is_dropped() {
        let nodes = anchor("JAVASCRIPT:alert(1)");
        let rendered = render_nodes_to_html(&nodes, None);
        assert!(!rendered.contains("href="));
    }

    #[test]
    fn whitespace_prefixed_javascript_href_is_dropped() {
        let nodes = anchor("\tjavascript:alert(1)");
        let rendered = render_nodes_to_html(&nodes, None);
        assert!(!rendered.contains("href="));
    }

    #[test]
    fn data_uri_in_img_src_is_dropped() {
        let nodes = img("data:text/html,<script>alert(1)</script>");
        let rendered = render_nodes_to_html(&nodes, None);
        assert!(!rendered.contains("src="));
        assert_eq!(rendered, "<img />");
    }

    #[test]
    fn vbscript_href_is_dropped() {
        let nodes = anchor("vbscript:msgbox('xss')");
        let rendered = render_nodes_to_html(&nodes, None);
        assert!(!rendered.contains("href="));
    }

    #[test]
    fn file_href_is_dropped() {
        let nodes = anchor("file:///etc/passwd");
        let rendered = render_nodes_to_html(&nodes, None);
        assert!(!rendered.contains("href="));
    }

    #[test]
    fn blob_href_is_dropped() {
        let nodes = anchor("blob:https://example.com/abc-123");
        let rendered = render_nodes_to_html(&nodes, None);
        assert!(!rendered.contains("href="));
    }

    #[test]
    fn protocol_relative_img_src_is_promoted_to_https() {
        let nodes = img("//cdn.example.com/image.jpg");
        let base = telegraph_base();
        let rendered = render_nodes_to_html(&nodes, Some(&base));
        assert!(rendered.contains("src=\"https://cdn.example.com/image.jpg\""));
    }

    #[test]
    fn absolute_path_href_is_resolved_against_telegraph_base() {
        let nodes = anchor("/Other-Post-01-01");
        let base = telegraph_base();
        let rendered = render_nodes_to_html(&nodes, Some(&base));
        assert!(rendered.contains("href=\"https://telegra.ph/Other-Post-01-01\""));
    }

    #[test]
    fn relative_path_img_src_is_resolved_against_telegraph_base() {
        let nodes = img("bar.jpg");
        let base = telegraph_base();
        let rendered = render_nodes_to_html(&nodes, Some(&base));
        assert!(rendered.contains("src=\"https://telegra.ph/bar.jpg\""));
    }

    #[test]
    fn mailto_href_is_preserved_on_anchor() {
        let nodes = anchor("mailto:user@example.com");
        let rendered = render_nodes_to_html(&nodes, None);
        assert!(rendered.contains("href=\"mailto:user@example.com\""));
    }

    #[test]
    fn tel_href_is_preserved_on_anchor() {
        let nodes = anchor("tel:+1234567890");
        let rendered = render_nodes_to_html(&nodes, None);
        assert!(rendered.contains("href=\"tel:+1234567890\""));
    }

    #[test]
    fn http_img_src_is_dropped_https_only() {
        let nodes = img("http://example.com/image.jpg");
        let rendered = render_nodes_to_html(&nodes, None);
        assert!(!rendered.contains("src="));
        assert_eq!(rendered, "<img />");
    }

    #[test]
    fn https_iframe_src_is_preserved_with_sandbox() {
        let mut attrs = HashMap::new();
        attrs.insert(
            "src".to_string(),
            "https://www.youtube.com/embed/abc".to_string(),
        );
        let nodes = vec![Node::Element(NodeElement {
            tag: "iframe".to_string(),
            attrs: Some(attrs),
            children: None,
        })];
        let rendered = render_nodes_to_html(&nodes, None);
        assert!(rendered.contains("src=\"https://www.youtube.com/embed/abc\""));
        assert!(rendered.contains("sandbox"));
    }

    #[test]
    fn missing_base_drops_relative_but_keeps_absolute_https() {
        let relative = anchor("/foo");
        let dropped = render_nodes_to_html(&relative, None);
        assert!(!dropped.contains("href="));

        let absolute = anchor("https://example.com/path");
        let kept = render_nodes_to_html(&absolute, None);
        assert!(kept.contains("href=\"https://example.com/path\""));
    }
}
