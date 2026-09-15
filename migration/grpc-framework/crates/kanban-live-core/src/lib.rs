//! 可重建的 UI 投影、有限历史和 source 接口；不依赖 HTTP、Protobuf 或数据库。
mod hub;
mod model;
mod source;
pub use hub::*;
pub use model::*;
pub use source::*;
#[cfg(test)]
mod tests;
mod refresh;
pub use refresh::RefreshSource;
