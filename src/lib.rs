//! Library crate backing the Quaver player-stats card service.
//!
//! Exposes the building blocks used to fetch a Quaver player's data, fetch and
//! cache their avatar, render a stats card image, and serve it over HTTP with
//! an in-memory response cache.

/// Avatar fetching, on-disk caching, decoding and resizing.
pub mod avatar;
/// In-memory, TTL-based byte cache used for rendered card responses.
pub mod cache;
/// HTTP handler and image rendering for the player stats card.
pub mod card;
/// Quaver user data model and API fetching/parsing.
pub mod user;
