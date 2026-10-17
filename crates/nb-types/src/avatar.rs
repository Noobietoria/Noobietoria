/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Avatar customization: body colors, clothes (shirts & pants), and faces.

use serde::{Deserialize, Serialize};
use crate::math::Rgba;

/// Full avatar appearance configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AvatarConfig {
    /// Head part color
    pub head_color: Rgba,
    /// Torso part color
    pub torso_color: Rgba,
    /// Left arm color
    pub left_arm_color: Rgba,
    /// Right arm color
    pub right_arm_color: Rgba,
    /// Left leg color
    pub left_leg_color: Rgba,
    /// Right leg color
    pub right_leg_color: Rgba,
    /// Shirt identifier (e.g., "default", "hoodie", "tuxedo", "tee", "flannel", "cyber", "striped")
    pub shirt: String,
    /// Pants identifier (e.g., "default", "jeans", "cargo", "sweatpants", "shorts", "slacks")
    pub pants: String,
    /// Face identifier (e.g., "smile", "wink", "smirk", "chill", "joy", "silly", "surprised", "ninja")
    pub face: String,
    /// Scale height (1.0 = standard)
    pub height_scale: f32,
    /// Scale width (1.0 = standard)
    pub width_scale: f32,
}

impl Default for AvatarConfig {
    fn default() -> Self {
        Self::classic_noob()
    }
}

impl AvatarConfig {
    /// Classic Roblox Noob: Yellow head/arms, Blue torso, Green legs, Classic Smile.
    pub fn classic_noob() -> Self {
        Self {
            head_color: Rgba::new(245.0, 205.0, 48.0, 255.0),      // Bright yellow
            torso_color: Rgba::new(13.0, 105.0, 172.0, 255.0),     // Bright blue
            left_arm_color: Rgba::new(245.0, 205.0, 48.0, 255.0),  // Bright yellow
            right_arm_color: Rgba::new(245.0, 205.0, 48.0, 255.0), // Bright yellow
            left_leg_color: Rgba::new(164.0, 189.0, 71.0, 255.0),  // Bright yellowish green
            right_leg_color: Rgba::new(164.0, 189.0, 71.0, 255.0), // Bright yellowish green
            shirt: "default".to_owned(),
            pants: "default".to_owned(),
            face: "smile".to_owned(),
            height_scale: 1.0,
            width_scale: 1.0,
        }
    }

    /// Retro Gamer preset
    pub fn retro_gamer() -> Self {
        Self {
            head_color: Rgba::new(255.0, 220.0, 180.0, 255.0),
            torso_color: Rgba::new(220.0, 50.0, 50.0, 255.0),
            left_arm_color: Rgba::new(255.0, 220.0, 180.0, 255.0),
            right_arm_color: Rgba::new(255.0, 220.0, 180.0, 255.0),
            left_leg_color: Rgba::new(40.0, 60.0, 100.0, 255.0),
            right_leg_color: Rgba::new(40.0, 60.0, 100.0, 255.0),
            shirt: "hoodie".to_owned(),
            pants: "jeans".to_owned(),
            face: "chill".to_owned(),
            height_scale: 1.0,
            width_scale: 1.0,
        }
    }

    /// Cyber Ninja preset
    pub fn cyber_ninja() -> Self {
        Self {
            head_color: Rgba::new(40.0, 40.0, 45.0, 255.0),
            torso_color: Rgba::new(20.0, 20.0, 25.0, 255.0),
            left_arm_color: Rgba::new(0.0, 200.0, 255.0, 255.0),
            right_arm_color: Rgba::new(0.0, 200.0, 255.0, 255.0),
            left_leg_color: Rgba::new(30.0, 30.0, 35.0, 255.0),
            right_leg_color: Rgba::new(30.0, 30.0, 35.0, 255.0),
            shirt: "cyber".to_owned(),
            pants: "cargo".to_owned(),
            face: "ninja".to_owned(),
            height_scale: 1.0,
            width_scale: 1.0,
        }
    }

    /// Sets all skin/body parts to a uniform color.
    pub fn set_uniform_color(&mut self, color: Rgba) {
        self.head_color = color;
        self.torso_color = color;
        self.left_arm_color = color;
        self.right_arm_color = color;
        self.left_leg_color = color;
        self.right_leg_color = color;
    }
}

/// Catalog item category in the Avatar Shop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AvatarCategory {
    BodyColor,
    Shirt,
    Pants,
    Face,
}

impl AvatarCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::BodyColor => "Body Colors",
            Self::Shirt => "Shirts",
            Self::Pants => "Pants",
            Self::Face => "Faces",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::BodyColor => "🎨",
            Self::Shirt => "👕",
            Self::Pants => "👖",
            Self::Face => "😀",
        }
    }
}

/// One purchasable / equippable item in the Avatar Shop.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogItem {
    pub id: String,
    pub name: String,
    pub category: AvatarCategory,
    pub price_studs: u32,
    pub description: String,
    pub icon: String,
}

/// Pre-populated catalog items available in the Noobietoria Avatar Shop.
pub fn default_catalog() -> Vec<CatalogItem> {
    vec![
        // ── Faces ─────────────────────────────────────────────────────────────
        CatalogItem {
            id: "smile".to_owned(),
            name: "Classic Smile".to_owned(),
            category: AvatarCategory::Face,
            price_studs: 0,
            description: "The timeless, friendly smile that started it all.".to_owned(),
            icon: "🙂".to_owned(),
        },
        CatalogItem {
            id: "wink".to_owned(),
            name: "Playful Wink".to_owned(),
            category: AvatarCategory::Face,
            price_studs: 5,
            description: "A cheeky wink for your victorious moments.".to_owned(),
            icon: "😉".to_owned(),
        },
        CatalogItem {
            id: "smirk".to_owned(),
            name: "Confident Smirk".to_owned(),
            category: AvatarCategory::Face,
            price_studs: 5,
            description: "Show everyone you already know the answer.".to_owned(),
            icon: "😏".to_owned(),
        },
        CatalogItem {
            id: "chill".to_owned(),
            name: "Chill Shades".to_owned(),
            category: AvatarCategory::Face,
            price_studs: 10,
            description: "Stay cool under pressure with tinted sunglasses.".to_owned(),
            icon: "😎".to_owned(),
        },
        CatalogItem {
            id: "joy".to_owned(),
            name: "Joyful Grin".to_owned(),
            category: AvatarCategory::Face,
            price_studs: 5,
            description: "Uncontrollable happiness across your blocky face.".to_owned(),
            icon: "😄".to_owned(),
        },
        CatalogItem {
            id: "silly".to_owned(),
            name: "Silly Tongue".to_owned(),
            category: AvatarCategory::Face,
            price_studs: 10,
            description: "Blehhhh! Stick your tongue out at gravity.".to_owned(),
            icon: "😜".to_owned(),
        },
        CatalogItem {
            id: "surprised".to_owned(),
            name: "Surprised O_O".to_owned(),
            category: AvatarCategory::Face,
            price_studs: 5,
            description: "When the baseplate drops out from under you.".to_owned(),
            icon: "😮".to_owned(),
        },
        CatalogItem {
            id: "ninja".to_owned(),
            name: "Shadow Ninja".to_owned(),
            category: AvatarCategory::Face,
            price_studs: 15,
            description: "Stealthy mask covering everything except sharp eyes.".to_owned(),
            icon: "🥷".to_owned(),
        },

        // ── Shirts ────────────────────────────────────────────────────────────
        CatalogItem {
            id: "default".to_owned(),
            name: "Classic Solid Tee".to_owned(),
            category: AvatarCategory::Shirt,
            price_studs: 0,
            description: "Clean solid color matches your torso tint.".to_owned(),
            icon: "👕".to_owned(),
        },
        CatalogItem {
            id: "noobietoria_tee".to_owned(),
            name: "Noobietoria Official Tee".to_owned(),
            category: AvatarCategory::Shirt,
            price_studs: 5,
            description: "Features the iconic Noobietoria rose square badge.".to_owned(),
            icon: "🎽".to_owned(),
        },
        CatalogItem {
            id: "hoodie".to_owned(),
            name: "Cozy Zip Hoodie".to_owned(),
            category: AvatarCategory::Shirt,
            price_studs: 10,
            description: "Warm hoodie with zipper line and front pockets.".to_owned(),
            icon: "🧥".to_owned(),
        },
        CatalogItem {
            id: "tuxedo".to_owned(),
            name: "Gentleman Tuxedo".to_owned(),
            category: AvatarCategory::Shirt,
            price_studs: 15,
            description: "Formal black suit jacket with bowtie and collar.".to_owned(),
            icon: "🤵".to_owned(),
        },
        CatalogItem {
            id: "flannel".to_owned(),
            name: "Red Lumberjack Flannel".to_owned(),
            category: AvatarCategory::Shirt,
            price_studs: 10,
            description: "Rustic checkered pattern for brave builders.".to_owned(),
            icon: "👔".to_owned(),
        },
        CatalogItem {
            id: "cyber".to_owned(),
            name: "Cyber Neon Exosuit".to_owned(),
            category: AvatarCategory::Shirt,
            price_studs: 20,
            description: "Futuristic armor plating glowing with cyan circuits.".to_owned(),
            icon: "🛡️".to_owned(),
        },
        CatalogItem {
            id: "striped".to_owned(),
            name: "Rainbow Striped Sweater".to_owned(),
            category: AvatarCategory::Shirt,
            price_studs: 10,
            description: "Vibrant horizontal stripes with retro charm.".to_owned(),
            icon: "🧶".to_owned(),
        },

        // ── Pants ─────────────────────────────────────────────────────────────
        CatalogItem {
            id: "default".to_owned(),
            name: "Classic Solid Pants".to_owned(),
            category: AvatarCategory::Pants,
            price_studs: 0,
            description: "Clean solid finish tinted by your leg colors.".to_owned(),
            icon: "👖".to_owned(),
        },
        CatalogItem {
            id: "jeans".to_owned(),
            name: "Vintage Blue Jeans".to_owned(),
            category: AvatarCategory::Pants,
            price_studs: 5,
            description: "Classic denim jeans with stitched pocket outlines.".to_owned(),
            icon: "👖".to_owned(),
        },
        CatalogItem {
            id: "cargo".to_owned(),
            name: "Tactical Cargo Pants".to_owned(),
            category: AvatarCategory::Pants,
            price_studs: 10,
            description: "Dark tactical trousers loaded with side pockets.".to_owned(),
            icon: "🩳".to_owned(),
        },
        CatalogItem {
            id: "sweatpants".to_owned(),
            name: "White Sport Sweatpants".to_owned(),
            category: AvatarCategory::Pants,
            price_studs: 10,
            description: "Comfy athletic sweatpants with dual side stripes.".to_owned(),
            icon: "🩳".to_owned(),
        },
        CatalogItem {
            id: "slacks".to_owned(),
            name: "Formal Black Slacks".to_owned(),
            category: AvatarCategory::Pants,
            price_studs: 15,
            description: "Pressed dress trousers to match a tuxedo.".to_owned(),
            icon: "👖".to_owned(),
        },
        CatalogItem {
            id: "shorts".to_owned(),
            name: "Summer Boardshorts".to_owned(),
            category: AvatarCategory::Pants,
            price_studs: 5,
            description: "Breezy shorts perfect for a tropical baseplate.".to_owned(),
            icon: "🩳".to_owned(),
        },

        // ── Body Color Presets ────────────────────────────────────────────────
        CatalogItem {
            id: "preset_noob".to_owned(),
            name: "Classic Noob Palette".to_owned(),
            category: AvatarCategory::BodyColor,
            price_studs: 0,
            description: "Yellow head and arms, blue torso, green legs.".to_owned(),
            icon: "🟡".to_owned(),
        },
        CatalogItem {
            id: "preset_skin".to_owned(),
            name: "Natural Skin Tone".to_owned(),
            category: AvatarCategory::BodyColor,
            price_studs: 0,
            description: "Warm natural peach skin tone across all limbs.".to_owned(),
            icon: "🧑".to_owned(),
        },
        CatalogItem {
            id: "preset_dark".to_owned(),
            name: "Deep Obsidian".to_owned(),
            category: AvatarCategory::BodyColor,
            price_studs: 5,
            description: "Sleek charcoal and dark grey body scheme.".to_owned(),
            icon: "⚫".to_owned(),
        },
        CatalogItem {
            id: "preset_gold".to_owned(),
            name: "Pure Solid Gold".to_owned(),
            category: AvatarCategory::BodyColor,
            price_studs: 25,
            description: "Gleaming golden luster for wealthy players.".to_owned(),
            icon: "🏆".to_owned(),
        },
    ]
}
