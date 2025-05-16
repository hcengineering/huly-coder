//! Modified version of rig::agent::providers::openrouter
//! OpenRouter Inference API client and Rig integration
//!
//! # Example
//! ```
//! use rig::providers::openrouter;
//!
//! let client = openrouter::Client::new("YOUR_API_KEY");
//!
//! let llama_3_1_8b = client.completion_model(openrouter::LLAMA_3_1_8B);
//! ```

pub mod client;
pub mod completion;
pub mod streaming;

pub use client::*;
pub use completion::*;

pub fn merge(a: serde_json::Value, b: serde_json::Value) -> serde_json::Value {
    match (a, b) {
        (serde_json::Value::Object(mut a_map), serde_json::Value::Object(b_map)) => {
            b_map.into_iter().for_each(|(key, value)| {
                a_map.insert(key, value);
            });
            serde_json::Value::Object(a_map)
        }
        (a, _) => a,
    }
}
