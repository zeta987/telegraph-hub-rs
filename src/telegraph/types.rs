use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Generic Telegraph API response wrapper.
/// All Telegraph endpoints return `{ "ok": bool, "result": T }` on success
/// or `{ "ok": false, "error": "..." }` on failure.
#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub ok: bool,
    pub result: Option<T>,
    pub error: Option<String>,
}

/// Telegraph account information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub short_name: Option<String>,
    pub author_name: Option<String>,
    pub author_url: Option<String>,
    pub access_token: Option<String>,
    pub auth_url: Option<String>,
    pub page_count: Option<i64>,
}

/// A Telegraph page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub path: String,
    pub url: String,
    pub title: String,
    pub description: Option<String>,
    pub author_name: Option<String>,
    pub author_url: Option<String>,
    pub image_url: Option<String>,
    pub content: Option<Vec<Node>>,
    pub views: i64,
    pub can_edit: Option<bool>,
}

/// Paginated list of Telegraph pages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageList {
    pub total_count: i64,
    pub pages: Vec<Page>,
}

/// Page view statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageViews {
    pub views: i64,
}

/// A Telegraph content node.
///
/// Telegraph content is represented as an array of `Node` values.
/// Each node is either a plain text string or a structured element
/// with a tag, optional attributes, and optional children.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Node {
    Text(String),
    Element(NodeElement),
}

/// A structured Telegraph content element.
///
/// Allowed tags: a, aside, b, blockquote, br, code, em, figcaption,
/// figure, h3, h4, hr, i, iframe, img, li, ol, p, pre, s, strong, u, ul, video.
///
/// Allowed attributes: href, src.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeElement {
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attrs: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<Node>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_text_node() {
        let json = r#""Hello, world!""#;
        let node: Node = serde_json::from_str(json).unwrap();
        match node {
            Node::Text(s) => assert_eq!(s, "Hello, world!"),
            _ => panic!("expected Text node"),
        }
    }

    #[test]
    fn deserialize_element_node() {
        let json = r#"{"tag":"p","children":["Hello"]}"#;
        let node: Node = serde_json::from_str(json).unwrap();
        match node {
            Node::Element(el) => {
                assert_eq!(el.tag, "p");
                assert!(el.attrs.is_none());
                let children = el.children.unwrap();
                assert_eq!(children.len(), 1);
                match &children[0] {
                    Node::Text(s) => assert_eq!(s, "Hello"),
                    _ => panic!("expected Text child"),
                }
            }
            _ => panic!("expected Element node"),
        }
    }

    #[test]
    fn deserialize_link_node() {
        let json = r#"{"tag":"a","attrs":{"href":"https://example.com"},"children":["click"]}"#;
        let node: Node = serde_json::from_str(json).unwrap();
        match node {
            Node::Element(el) => {
                assert_eq!(el.tag, "a");
                let attrs = el.attrs.unwrap();
                assert_eq!(attrs.get("href").unwrap(), "https://example.com");
            }
            _ => panic!("expected Element node"),
        }
    }

    #[test]
    fn serialize_node_roundtrip() {
        let nodes = vec![
            Node::Element(NodeElement {
                tag: "p".to_string(),
                attrs: None,
                children: Some(vec![Node::Text("Hello".to_string())]),
            }),
            Node::Element(NodeElement {
                tag: "p".to_string(),
                attrs: None,
                children: Some(vec![Node::Text("World".to_string())]),
            }),
        ];
        let json = serde_json::to_string(&nodes).unwrap();
        let parsed: Vec<Node> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn deserialize_api_response_success() {
        let json = r#"{"ok":true,"result":{"short_name":"Test","author_name":"Author"}}"#;
        let resp: ApiResponse<Account> = serde_json::from_str(json).unwrap();
        assert!(resp.ok);
        assert!(resp.result.is_some());
        assert_eq!(resp.result.unwrap().short_name.unwrap(), "Test");
    }

    #[test]
    fn deserialize_api_response_error() {
        let json = r#"{"ok":false,"error":"INVALID_TOKEN"}"#;
        let resp: ApiResponse<Account> = serde_json::from_str(json).unwrap();
        assert!(!resp.ok);
        assert_eq!(resp.error.unwrap(), "INVALID_TOKEN");
    }

    #[test]
    fn deserialize_page_list() {
        let json = r#"{"total_count":1,"pages":[{"path":"test-01-01","url":"https://telegra.ph/test-01-01","title":"Test","views":42}]}"#;
        let list: PageList = serde_json::from_str(json).unwrap();
        assert_eq!(list.total_count, 1);
        assert_eq!(list.pages[0].title, "Test");
        assert_eq!(list.pages[0].views, 42);
    }
}
