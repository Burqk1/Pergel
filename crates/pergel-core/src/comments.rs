//! Comments: threads pinned to the canvas. A thread dropped on a layer belongs to it (it moves
//! with the layer and is saved inside it); one on empty canvas is kept on its page.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    pub author: String,
    pub text: String,
    /// Milliseconds since 1970.
    #[serde(default)]
    pub time: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommentThread {
    pub id: String,
    /// Relative to the layer's top-left corner (layer threads) or the page (page threads).
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub resolved: bool,
    pub messages: Vec<Comment>,
}

/// Team libraries: the library component a local main came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryRef {
    /// The library (its file id), the component's key in it, and the published version used.
    pub lib: String,
    pub key: String,
    pub version: f64,
    #[serde(default)]
    pub lib_name: String,
}

/// A thread on empty canvas, with the page it is on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageComment {
    pub page: usize,
    #[serde(flatten)]
    pub thread: CommentThread,
}
