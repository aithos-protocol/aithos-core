//! LLM proxy — post-MVP stub.
//!
//! Decided 2026-07-10: v1 targets the OpenAI-compatible API (the widest
//! coverage: OpenAI, vLLM, Ollama, Mistral, Groq…). The gateway will hold
//! the provider credentials (vault `/x/`), impose the model, read the real
//! `usage`, enforce token budgets, and log one `inference` gamma entry per
//! call — metadata only, never the prompt (that stays in the agent cache).
//!
//! Nothing here is wired yet; the type exists so the component map of
//! GATEWAY-BOOTSTRAP §4 is visible in code.

/// Placeholder for the OpenAI-compatible proxy (post-MVP).
pub struct LlmProxy;
