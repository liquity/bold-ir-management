//! Strategy Core Systems
//!
//! A comprehensive framework for executing and managing automated interest rate strategies.
//! This module orchestrates strategy lifecycle, state management, and execution flow
//! through several coordinated components.
//!
//! ```plain
//! Strategy Architecture:
//!
//! ┌──────────┐     ┌──────────┐    ┌──────────┐
//! │          │     │          │    │          │
//! │ Settings ├────►│  Runner  │◄───┤   Data   │
//! │          │     │          │    │          │
//! └──────────┘     └────┬─────┘    └──────────┘
//!                       │
//!                       ▼
//!              ┌───────────────┐
//!              │               │
//!        ┌─────┤  Strategies   ├─────┐
//!        │     │               │     │
//!        │     └───────────────┘     │
//!        ▼                           ▼
//!   ┌─────────┐                 ┌───────────┐
//!   │  Stable │◄───────────────►│Executable │
//!   │Strategy │      Lock       │Strategy   │
//!   └─────────┘                 └───────────┘
//! ```
//!
//! Module Components:
//!
//! - `data`: Strategy runtime state management
//! - `run`: Strategy execution orchestration
//! - `settings`: Strategy configuration parameters
//! - `stable`: Persistent strategy storage
//! - `executable`: Runtime strategy operations
//! - `lock`: Concurrent execution control
//!
//! Key Design Features:
//!
//! 1. Clear separation between storage and execution
//! 2. Controlled visibility of executable components
//! 3. Coordinated state transitions
//! 4. Safe concurrent execution
//!
//! Note: Executable strategy access is intentionally restricted to ensure
//! proper lifecycle management and state consistency.

// Core component modules
pub(crate) mod data; // Strategy state
pub(crate) mod run; // Execution flow
pub(crate) mod settings; // Configuration
pub(crate) mod stable; // Persistent storage

// Restricted access modules
pub(in crate::strategy) mod executable; // Runtime execution
pub(in crate::strategy) mod lock; // Concurrency control
