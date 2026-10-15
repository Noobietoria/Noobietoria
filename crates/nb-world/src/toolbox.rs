/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Toolbox Catalog: Built-in Roblox-style models, interactive props,
//! hazard bricks, speed pads, pickups, NPCs and lighting presets.

use nb_types::{Container, InstanceType, Rgba, Vector3};

use crate::instance::{Instance, InstanceId, InstanceProperties};
use crate::world::{World, WorldError, WorldResult};

/// Represents an insertable asset in the Noobietoria Toolbox.
#[derive(Clone, Copy, Debug)]
pub struct ToolboxAsset {
    /// Unique identifier for this asset.
    pub id: &'static str,
    /// Display name shown in the Toolbox search results.
    pub name: &'static str,
    /// Category: "Models", "Parts", "Scripts", "Lighting".
    pub category: &'static str,
    /// Creator attribution.
    pub author: &'static str,
    /// Brief description of the asset functionality.
    pub description: &'static str,
    /// Visual icon or emoji for UI display.
    pub icon: &'static str,
    /// Generator function creating instances at a given origin.
    pub instantiate: fn(origin: Vector3) -> Vec<Instance>,
}

fn make_part(
    name: &str,
    pos: Vector3,
    size: Vector3,
    color: Rgba,
    parent_name: Option<String>,
) -> Instance {
    let mut props = InstanceProperties::default();
    props.position = pos;
    props.size = size;
    props.color = color;
    Instance {
        id: InstanceId::new(),
        name: name.to_owned(),
        kind: InstanceType::Part,
        parent: Container::Workspace,
        parent_name,
        properties: props,
        created_at: 0,
    }
}

fn make_server_script(name: &str, source: &str, parent_name: Option<String>) -> Instance {
    let mut props = InstanceProperties::default();
    props.source = source.to_owned();
    Instance {
        id: InstanceId::new(),
        name: name.to_owned(),
        kind: InstanceType::ServerScript,
        parent: Container::Workspace,
        parent_name,
        properties: props,
        created_at: 0,
    }
}

// -----------------------------------------------------------------------------
// Built-in Asset Generators
// -----------------------------------------------------------------------------

fn gen_block(pos: Vector3) -> Vec<Instance> {
    vec![make_part(
        "Part",
        pos,
        Vector3::new(4.0, 1.2, 2.0),
        Rgba::new(163.0, 162.0, 165.0, 255.0),
        None,
    )]
}

fn gen_spawn_location(pos: Vector3) -> Vec<Instance> {
    let mut props = InstanceProperties::default();
    props.position = pos;
    props.size = Vector3::new(12.0, 1.0, 12.0);
    props.color = Rgba::new(196.0, 40.0, 28.0, 255.0); // Crimson spawn decal background
    vec![Instance {
        id: InstanceId::new(),
        name: "SpawnLocation".to_owned(),
        kind: InstanceType::SpawnLocation,
        parent: Container::Workspace,
        parent_name: None,
        properties: props,
        created_at: 0,
    }]
}

fn gen_lava_brick(pos: Vector3) -> Vec<Instance> {
    let block = make_part(
        "LavaBrick",
        pos,
        Vector3::new(6.0, 1.0, 6.0),
        Rgba::new(255.0, 0.0, 0.0, 255.0), // Bright neon red
        None,
    );
    let script = make_server_script(
        "KillScript",
        r#"-- Kill on touch
local part = script.Parent
part.Touched:Connect(function(hit)
    local humanoid = hit.Parent:FindFirstChild("Humanoid")
    if humanoid then
        humanoid.Health = 0
    end
end)
"#,
        Some("LavaBrick".to_owned()),
    );
    vec![block, script]
}

fn gen_speed_pad(pos: Vector3) -> Vec<Instance> {
    let pad = make_part(
        "SpeedPad",
        pos,
        Vector3::new(8.0, 0.4, 8.0),
        Rgba::new(0.0, 162.0, 232.0, 255.0), // Electric Cyan
        None,
    );
    let script = make_server_script(
        "SpeedScript",
        r#"-- 2x WalkSpeed boost
local pad = script.Parent
pad.Touched:Connect(function(hit)
    local humanoid = hit.Parent:FindFirstChild("Humanoid")
    if humanoid then
        humanoid.WalkSpeed = 50
    end
end)
"#,
        Some("SpeedPad".to_owned()),
    );
    vec![pad, script]
}

fn gen_jump_pad(pos: Vector3) -> Vec<Instance> {
    let pad = make_part(
        "JumpPad",
        pos,
        Vector3::new(6.0, 0.4, 6.0),
        Rgba::new(255.0, 215.0, 0.0, 255.0), // Gold neon
        None,
    );
    let script = make_server_script(
        "JumpScript",
        r#"-- Super Jump Boost
local pad = script.Parent
pad.Touched:Connect(function(hit)
    local humanoid = hit.Parent:FindFirstChild("Humanoid")
    if humanoid then
        humanoid.JumpPower = 120
    end
end)
"#,
        Some("JumpPad".to_owned()),
    );
    vec![pad, script]
}

fn gen_coin_pickup(pos: Vector3) -> Vec<Instance> {
    let coin = make_part(
        "Coin",
        pos,
        Vector3::new(2.0, 2.0, 0.4),
        Rgba::new(255.0, 204.0, 0.0, 255.0), // Shiny gold
        None,
    );
    let script = make_server_script(
        "CoinPickupScript",
        r#"-- Reward coins and destroy
local coin = script.Parent
coin.Touched:Connect(function(hit)
    local player = Players:GetPlayerFromCharacter(hit.Parent)
    if player then
        LeaderboardService:AddScore(player.UserId, "Coins", 10)
        coin:Destroy()
    end
end)
"#,
        Some("Coin".to_owned()),
    );
    vec![coin, script]
}

fn gen_door_tween(pos: Vector3) -> Vec<Instance> {
    let frame_left = make_part(
        "FrameLeft",
        Vector3::new(pos.x - 2.2, pos.y + 4.0, pos.z),
        Vector3::new(0.6, 8.0, 1.0),
        Rgba::new(100.0, 100.0, 100.0, 255.0),
        None,
    );
    let frame_right = make_part(
        "FrameRight",
        Vector3::new(pos.x + 2.2, pos.y + 4.0, pos.z),
        Vector3::new(0.6, 8.0, 1.0),
        Rgba::new(100.0, 100.0, 100.0, 255.0),
        None,
    );
    let door = make_part(
        "SlidingDoor",
        Vector3::new(pos.x, pos.y + 4.0, pos.z),
        Vector3::new(4.0, 7.8, 0.4),
        Rgba::new(139.0, 69.0, 19.0, 255.0), // Wood brown
        None,
    );
    let script = make_server_script(
        "DoorScript",
        r#"-- Automatic sliding door
local door = script.Parent
local open = false
door.Touched:Connect(function(hit)
    if not open then
        open = true
        door.Position = door.Position + Vector3.new(0, 8, 0)
        task.wait(3)
        door.Position = door.Position - Vector3.new(0, 8, 0)
        open = false
    end
end)
"#,
        Some("SlidingDoor".to_owned()),
    );
    vec![frame_left, frame_right, door, script]
}

fn gen_shopkeeper_npc(pos: Vector3) -> Vec<Instance> {
    let mut torso = make_part(
        "Shopkeeper",
        Vector3::new(pos.x, pos.y + 3.0, pos.z),
        Vector3::new(2.0, 2.0, 1.0),
        Rgba::new(13.0, 105.0, 172.0, 255.0), // Bright blue torso
        None,
    );
    torso.kind = InstanceType::Model;

    let head = make_part(
        "Head",
        Vector3::new(pos.x, pos.y + 4.5, pos.z),
        Vector3::new(1.2, 1.2, 1.2),
        Rgba::new(253.0, 234.0, 141.0, 255.0), // Classic yellow head
        Some("Shopkeeper".to_owned()),
    );
    let dialog_script = make_server_script(
        "ShopkeeperDialogue",
        r#"-- NPC Dialogue
local npc = script.Parent
npc.ClickDetector.MouseClick:Connect(function(player)
    DialogueService:StartDialogue("Shopkeeper", player.UserId)
end)
"#,
        Some("Shopkeeper".to_owned()),
    );
    vec![torso, head, dialog_script]
}

fn gen_checkpoint(pos: Vector3) -> Vec<Instance> {
    let pad = make_part(
        "Checkpoint",
        pos,
        Vector3::new(6.0, 0.4, 6.0),
        Rgba::new(245.0, 205.0, 47.0, 255.0),
        None,
    );
    let script = make_server_script(
        "CheckpointScript",
        r#"-- Save checkpoint stage
local pad = script.Parent
pad.Touched:Connect(function(hit)
    local player = Players:GetPlayerFromCharacter(hit.Parent)
    if player then
        LeaderboardService:SetScore(player.UserId, "Stage", 1)
    end
end)
"#,
        Some("Checkpoint".to_owned()),
    );
    vec![pad, script]
}

fn gen_day_night_script(_pos: Vector3) -> Vec<Instance> {
    let script = make_server_script(
        "DayNightCycle",
        r#"-- 24-minute Day and Night Cycle
while true do
    LightingService:SetMinutesAfterMidnight(LightingService:GetMinutesAfterMidnight() + 1)
    task.wait(1)
end
"#,
        None,
    );
    vec![script]
}

// -----------------------------------------------------------------------------
// Catalog Registry
// -----------------------------------------------------------------------------

/// The complete static catalog of built-in Toolbox assets.
pub static BUILTIN_CATALOG: &[ToolboxAsset] = &[
    ToolboxAsset {
        id: "basic_block",
        name: "Standard Part",
        category: "Parts",
        author: "Noobietoria",
        description: "Standard gray construction part (4x1.2x2).",
        icon: "🧱",
        instantiate: gen_block,
    },
    ToolboxAsset {
        id: "spawn_location",
        name: "Spawn Location",
        category: "Models",
        author: "Noobietoria",
        description: "Player respawn platform with decal.",
        icon: "🚩",
        instantiate: gen_spawn_location,
    },
    ToolboxAsset {
        id: "lava_brick",
        name: "Lava Kill Brick",
        category: "Models",
        author: "Noobietoria",
        description: "Hazard neon red brick that eliminates player on contact.",
        icon: "🔥",
        instantiate: gen_lava_brick,
    },
    ToolboxAsset {
        id: "speed_pad",
        name: "Speed Booster Pad",
        category: "Models",
        author: "Noobietoria",
        description: "Electric cyan pad increasing player WalkSpeed to 50.",
        icon: "⚡",
        instantiate: gen_speed_pad,
    },
    ToolboxAsset {
        id: "jump_pad",
        name: "Super Jump Pad",
        category: "Models",
        author: "Noobietoria",
        description: "Gold pad launching player high into the air.",
        icon: "🚀",
        instantiate: gen_jump_pad,
    },
    ToolboxAsset {
        id: "coin_pickup",
        name: "Collectible Coin",
        category: "Models",
        author: "Noobietoria",
        description: "Spinning gold coin awarding +10 currency on pickup.",
        icon: "🪙",
        instantiate: gen_coin_pickup,
    },
    ToolboxAsset {
        id: "sliding_door",
        name: "Sliding Door",
        category: "Models",
        author: "Noobietoria",
        description: "Automated wooden sliding door triggered on touch.",
        icon: "🚪",
        instantiate: gen_door_tween,
    },
    ToolboxAsset {
        id: "shopkeeper_npc",
        name: "Shopkeeper NPC",
        category: "Models",
        author: "Noobietoria",
        description: "Classic NPC with yellow head, blue torso and dialogue.",
        icon: "👤",
        instantiate: gen_shopkeeper_npc,
    },
    ToolboxAsset {
        id: "checkpoint_pad",
        name: "Stage Checkpoint",
        category: "Models",
        author: "Noobietoria",
        description: "Obby stage checkpoint recording player progress.",
        icon: "🏁",
        instantiate: gen_checkpoint,
    },
    ToolboxAsset {
        id: "day_night_script",
        name: "Day/Night Cycle Script",
        category: "Scripts",
        author: "Noobietoria",
        description: "Server script smoothly cycling time in LightingService.",
        icon: "☀️",
        instantiate: gen_day_night_script,
    },
];

/// Catalog helper functions.
pub struct ToolboxCatalog;

impl ToolboxCatalog {
    /// Returns all available toolbox assets.
    pub fn all() -> &'static [ToolboxAsset] {
        BUILTIN_CATALOG
    }

    /// Finds an asset by its unique ID.
    pub fn find_by_id(id: &str) -> Option<&'static ToolboxAsset> {
        BUILTIN_CATALOG.iter().find(|a| a.id == id)
    }

    /// Filters assets by keyword and optional category.
    pub fn search(query: &str, category: Option<&str>) -> Vec<&'static ToolboxAsset> {
        let q = query.trim().to_lowercase();
        BUILTIN_CATALOG
            .iter()
            .filter(|a| {
                if let Some(cat) = category {
                    if !cat.is_empty()
                        && !cat.eq_ignore_ascii_case("all")
                        && !a.category.eq_ignore_ascii_case(cat)
                    {
                        return false;
                    }
                }
                if q.is_empty() {
                    return true;
                }
                a.name.to_lowercase().contains(&q)
                    || a.description.to_lowercase().contains(&q)
                    || a.category.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Instantiates and inserts a toolbox asset into a [`World`] at the specified origin.
    pub fn insert_into_world(
        world: &mut World,
        asset_id: &str,
        origin: Vector3,
    ) -> WorldResult<Vec<InstanceId>> {
        let asset = Self::find_by_id(asset_id).ok_or_else(|| WorldError::NotFound {
            name: asset_id.to_owned(),
            container: Container::Workspace,
            index: None,
        })?;

        let instances = (asset.instantiate)(origin);
        let mut ids = Vec::with_capacity(instances.len());
        for instance in instances {
            ids.push(world.insert_instance(instance));
        }
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_items_exist() {
        assert!(BUILTIN_CATALOG.len() >= 10);
        assert!(ToolboxCatalog::find_by_id("lava_brick").is_some());
        assert!(ToolboxCatalog::find_by_id("speed_pad").is_some());
        assert!(ToolboxCatalog::find_by_id("coin_pickup").is_some());
    }

    #[test]
    fn search_filtering_works() {
        let models = ToolboxCatalog::search("", Some("Models"));
        assert!(!models.is_empty());
        for item in models {
            assert_eq!(item.category, "Models");
        }

        let speed = ToolboxCatalog::search("speed", None);
        assert_eq!(speed.len(), 1);
        assert_eq!(speed[0].id, "speed_pad");
    }

    #[test]
    fn insert_into_world_adds_instances() {
        let mut world = World::new();
        let origin = Vector3::new(10.0, 5.0, 10.0);
        let ids = ToolboxCatalog::insert_into_world(&mut world, "lava_brick", origin)
            .expect("insertion failed");
        assert_eq!(ids.len(), 2);
        assert_eq!(world.len(), 2);

        let part = world.find("LavaBrick", None).expect("LavaBrick not found");
        assert_eq!(part.properties.position, origin);
        assert_eq!(part.properties.color, Rgba::new(255.0, 0.0, 0.0, 255.0));
    }
}
