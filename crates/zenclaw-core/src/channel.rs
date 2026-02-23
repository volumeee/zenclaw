//! Channel trait — abstraction for chat platform adapters.

use async_trait::async_trait;

use crate::error::Result;
use crate::message::{Channel, OutboundMessage};

/// Channel adapter trait — implement this for each chat platform.
///
/// # Example
///
/// ```rust,ignore
/// struct MyChannel;
///
/// #[async_trait]
/// impl ChannelAdapter for MyChannel {
///     fn channel_type(&self) -> Channel { Channel::Telegram }
///     async fn start(&mut self) -> Result<()> { /* connect */ Ok(()) }
///     async fn stop(&mut self) -> Result<()> { /* disconnect */ Ok(()) }
///     async fn send(&self, msg: OutboundMessage) -> Result<()> { /* send */ Ok(()) }
/// }
/// ```
#[async_trait]
pub trait ChannelAdapter: Send + Sync {
    /// Channel type identifier.
    fn channel_type(&self) -> Channel;

    /// Start listening for messages. Should run in background.
    async fn start(&mut self) -> Result<()>;

    /// Stop the channel gracefully.
    async fn stop(&mut self) -> Result<()>;

    /// Send a message through this channel.
    async fn send(&self, msg: OutboundMessage) -> Result<()>;

    /// Check if a sender is allowed.
    fn is_allowed(&self, _sender_id: &str) -> bool {
        true // Default: allow everyone
    }

    /// Check if the channel is running.
    fn is_running(&self) -> bool;
}
