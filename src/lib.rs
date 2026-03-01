mod node;
mod parser;
mod renderer;

pub use node::Node;
pub use parser::parse;
pub use renderer::render;

/// 마크다운 스펙
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Spec {
    #[default]
    CommonMark,
    Gfm,
}
