/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Noobietoria World (`.nbw`) file format: serialization, deserialization,
//! place metadata and environment settings.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use nb_types::{Container, InstanceType, Rgba, Vector3};
use serde::{Deserialize, Serialize};

use crate::instance::{Instance, InstanceId, InstanceProperties};
use crate::world::World;

/// Magic header bytes for `.nbw` binary format: "NBW1"
pub const NBW_MAGIC: &[u8; 4] = b"NBW1";

/// Place metadata saved with the world file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceMetadata {
    /// Unique place ID or slug.
    pub place_id: String,
    /// Display name of the place.
    pub name: String,
    /// Description of the game/place.
    pub description: String,
    /// Username of the place creator.
    pub creator: String,
    /// Save version number (increments on save).
    pub version: u32,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
    /// ISO 8601 last modified timestamp.
    pub updated_at: String,
}

impl Default for PlaceMetadata {
    fn default() -> Self {
        Self {
            place_id: "place_starter".to_owned(),
            name: "Starter Place".to_owned(),
            description: "Default Noobietoria world with baseplate and starter spawn.".to_owned(),
            creator: "Noobietoria Studio".to_owned(),
            version: 1,
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
        }
    }
}

/// Place environment and gameplay settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaceSettings {
    /// World gravity (studs/s^2), Roblox standard is ~196.2.
    pub gravity: f32,
    /// Maximum player capacity in a server.
    pub max_players: u32,
    /// Default player jump power.
    pub jump_power: f32,
    /// Default player walk speed.
    pub walk_speed: f32,
    /// Time of day in format "HH:MM:SS" (e.g. "14:00:00").
    pub time_of_day: String,
    /// Ambient lighting color [R, G, B] normalized 0.0 to 1.0.
    pub ambient_color: [f32; 3],
    /// Global brightness multiplier.
    pub brightness: f32,
}

impl Default for PlaceSettings {
    fn default() -> Self {
        Self {
            gravity: 196.2,
            max_players: 50,
            jump_power: 50.0,
            walk_speed: 16.0,
            time_of_day: "14:00:00".to_owned(),
            ambient_color: [0.5, 0.5, 0.5],
            brightness: 1.0,
        }
    }
}

/// A serialized Noobietoria World (`.nbw`) package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldFile {
    /// File schema version.
    pub format_version: u32,
    /// Metadata about this place.
    pub metadata: PlaceMetadata,
    /// Global world and environment settings.
    pub settings: PlaceSettings,
    /// Flattened list of all instances in this place.
    pub instances: Vec<Instance>,
}

impl WorldFile {
    /// Creates a new empty world file with default settings and metadata.
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let metadata = PlaceMetadata {
            name: name.clone(),
            ..PlaceMetadata::default()
        };
        Self {
            format_version: 1,
            metadata,
            settings: PlaceSettings::default(),
            instances: Vec::new(),
        }
    }

    /// Captures a snapshot of an active [`World`] into a [`WorldFile`].
    pub fn from_world(world: &World, metadata: PlaceMetadata, settings: PlaceSettings) -> Self {
        let mut instances: Vec<Instance> = world.iter().cloned().collect();
        instances.sort_by_key(|i| i.created_at);
        Self {
            format_version: 1,
            metadata,
            settings,
            instances,
        }
    }

    /// Clears and repopulates the given [`World`] with the contents of this file.
    pub fn apply_to_world(&self, world: &mut World) {
        world.clear();
        for instance in &self.instances {
            world.insert_instance(instance.clone());
        }
    }

    /// Reconstructs a [`World`] from this file.
    pub fn to_world(&self) -> World {
        let mut world = World::new();
        self.apply_to_world(&mut world);
        world
    }

    /// Generates the standard default starter place with Baseplate and SpawnLocation.
    pub fn default_starter_place() -> Self {
        let mut file = Self::new("Baseplate Place");
        file.metadata.description = "A blank starter canvas with a baseplate and spawn.".to_owned();

        // 1. Baseplate part
        let mut baseplate_props = InstanceProperties::default();
        baseplate_props.position = Vector3::new(0.0, -0.5, 0.0);
        baseplate_props.size = Vector3::new(512.0, 1.0, 512.0);
        baseplate_props.color = Rgba::new(60.0, 64.0, 67.0, 255.0); // Dark slate grey
        file.instances.push(Instance {
            id: InstanceId::new(),
            name: "Baseplate".to_owned(),
            kind: InstanceType::Part,
            parent: Container::Workspace,
            parent_name: None,
            properties: baseplate_props,
            created_at: 0,
        });

        // 2. SpawnLocation part
        let mut spawn_props = InstanceProperties::default();
        spawn_props.position = Vector3::new(0.0, 0.5, 0.0);
        spawn_props.size = Vector3::new(12.0, 1.0, 12.0);
        spawn_props.color = Rgba::new(220.0, 225.0, 230.0, 255.0); // Light neutral grey
        file.instances.push(Instance {
            id: InstanceId::new(),
            name: "SpawnLocation".to_owned(),
            kind: InstanceType::SpawnLocation,
            parent: Container::Workspace,
            parent_name: None,
            properties: spawn_props,
            created_at: 1,
        });

        // 3. Ambient Music in Workspace
        let mut audio_props = InstanceProperties::default();
        audio_props.audio_id = "rbxasset://sounds/ambient_wind.mp3".to_owned();
        audio_props.volume = 0.5;
        audio_props.looping = true;
        file.instances.push(Instance {
            id: InstanceId::new(),
            name: "AmbientAudio".to_owned(),
            kind: InstanceType::Audio,
            parent: Container::Workspace,
            parent_name: None,
            properties: audio_props,
            created_at: 2,
        });

        file
    }

    /// Serializes the place to JSON format.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializes a place from JSON format.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serializes to binary `.nbw` format with header magic `NBW1`.
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        let json_bytes = serde_json::to_vec(self)?;
        let mut out = Vec::with_capacity(NBW_MAGIC.len() + json_bytes.len());
        out.extend_from_slice(NBW_MAGIC);
        out.extend_from_slice(&json_bytes);
        Ok(out)
    }

    /// Parses a `.nbw` file from bytes. Supports both magic header binary and raw JSON.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if bytes.starts_with(NBW_MAGIC) {
            let data = &bytes[NBW_MAGIC.len()..];
            let file: Self = serde_json::from_slice(data)?;
            Ok(file)
        } else {
            let file: Self = serde_json::from_slice(bytes)?;
            Ok(file)
        }
    }

    /// Saves the `.nbw` file to disk.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let data = self
            .to_bytes()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut file = File::create(path)?;
        file.write_all(&data)?;
        file.flush()?;
        Ok(())
    }

    /// Reads and deserializes a `.nbw` file from disk.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        Self::from_bytes(&buffer)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_starter_place_has_baseplate_and_spawn() {
        let file = WorldFile::default_starter_place();
        assert_eq!(file.format_version, 1);
        assert_eq!(file.metadata.name, "Baseplate Place");
        assert_eq!(file.instances.len(), 3);

        let world = file.to_world();
        assert_eq!(world.len(), 3);
        assert!(world.find("Baseplate", None).is_ok());
        assert!(world.find("SpawnLocation", None).is_ok());
        assert!(world.find("AmbientAudio", None).is_ok());
    }

    #[test]
    fn serialization_round_trip_bytes() {
        let starter = WorldFile::default_starter_place();
        let bytes = starter.to_bytes().expect("serialization to bytes failed");
        assert!(bytes.starts_with(NBW_MAGIC));

        let loaded = WorldFile::from_bytes(&bytes).expect("deserialization from bytes failed");
        assert_eq!(loaded.format_version, starter.format_version);
        assert_eq!(loaded.metadata.name, starter.metadata.name);
        assert_eq!(loaded.instances.len(), starter.instances.len());
        assert_eq!(loaded.settings.gravity, starter.settings.gravity);
    }

    #[test]
    fn serialization_round_trip_json() {
        let starter = WorldFile::default_starter_place();
        let json = starter.to_json().expect("serialization to json failed");
        let loaded = WorldFile::from_json(&json).expect("deserialization from json failed");
        assert_eq!(loaded.metadata.name, starter.metadata.name);
        assert_eq!(loaded.instances.len(), starter.instances.len());
    }

    #[test]
    fn file_save_and_load_round_trip() {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("test_save_load.nbw");

        let mut starter = WorldFile::default_starter_place();
        starter.metadata.name = "My Test Game".to_owned();
        starter.settings.gravity = 150.0;

        starter.save_to_file(&file_path).expect("save failed");
        let loaded = WorldFile::load_from_file(&file_path).expect("load failed");

        assert_eq!(loaded.metadata.name, "My Test Game");
        assert_eq!(loaded.settings.gravity, 150.0);
        assert_eq!(loaded.instances.len(), 3);

        let _ = std::fs::remove_file(file_path);
    }
}
