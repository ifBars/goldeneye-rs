//! Independent asset readers and runtime systems. Game data stays outside this crate.
pub mod actor_definitions;
pub mod animation_modes;
mod binary;
pub mod candidates;
pub mod character;
pub mod coarse;
pub mod collision;
pub mod disc;
pub mod edb;
pub mod entity_spatial;
pub mod filelist;
pub mod frontend_font;
pub mod frontend_morph;
pub mod frontend_script;
pub mod frontend_stream;
pub mod frontend_text;
pub mod geometry;
pub mod graph;
pub mod map_animator;
pub mod mission;
pub mod navigation;
pub mod player_static;
pub mod portals;
mod reference;
pub mod resource;
pub mod scene;
pub mod shape_bounds;
pub mod skeletal_animation;
pub mod skin;
pub mod sound_bank;
pub mod sound_info;
pub mod spatial;
pub mod spreadsheet;
pub mod supplied_data;
pub mod target_section;
pub mod thp;
pub mod tpl;
pub mod transform;
pub mod trigger_classes;
pub mod weapon_metadata;
pub mod weapon_static;

#[cfg(test)]
#[path = "tests/fixtures.rs"]
mod fixtures;
