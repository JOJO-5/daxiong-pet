//! Runs the production engine modules directly without installing a desktop WebView.
#![allow(dead_code)]
#[path = "../../../src-tauri/src/atlas.rs"]
mod atlas;
#[path = "../../../src-tauri/src/engine.rs"]
mod engine;
#[path = "../../../src-tauri/src/activities.rs"]
mod activities;
#[path = "../../../src-tauri/src/encounters.rs"]
mod encounters;
#[path = "../../../src-tauri/src/play.rs"]
mod play;
#[path = "../../../src-tauri/src/tug.rs"]
mod tug;
