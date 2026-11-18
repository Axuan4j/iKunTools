//! iKunTools 核心层。
//!
//! 设计约束：这里不许出现任何 UI / Tauri / 命令行相关的依赖。
//! 所有能力都是「输入 → 输出」的纯函数，方便被桌面端、CLI、CI 共用。

pub mod crypto;
pub mod encoding;
pub mod http;
pub mod json;
pub mod net;
pub mod jwt;
pub mod random;
pub mod regexp;
pub mod rsa;
pub mod timeutil;
pub mod xml;

#[derive(Debug, thiserror::Error)]
pub enum IktError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, IktError>;
