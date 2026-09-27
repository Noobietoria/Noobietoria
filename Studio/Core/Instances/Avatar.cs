using System;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// An item available on the Noobietoria Marketplace. Only Items and
    /// Clothes are Accessory kinds that can be equipped — Gamepasses cannot
    /// be equipped at all.
    ///
    /// Equipping happens on the wearer, not on the Accessory: set the
    /// IngameAccessoriesID property of a Player or NPC to a comma-separated
    /// string of Accessory IDs. No purchase is required to equip on a
    /// player; DefaultAccessories on the Player root only toggles
    /// already-equipped items.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#accessory
    /// </summary>
    public class Accessory : Instance
    {
        public Accessory(string name, string rootContainer)
            : base(name, "Accessory", rootContainer)
        {
        }

        /// <summary>
        /// Marketplace kind of this accessory: "Item" or "Clothes" (both
        /// equippable). "Gamepass" entries exist in the marketplace but can
        /// never be equipped.
        /// </summary>
        public string? Kind
        {
            get => GetProperty<string?>("Kind");
            set => SetProperty("Kind", value);
        }

        /// <summary>
        /// Marketplace item ID referenced by the wearer's
        /// IngameAccessoriesID list.
        /// </summary>
        public string? AccessoryId
        {
            get => GetProperty<string?>("AccessoryId");
            set => SetProperty("AccessoryId", value);
        }

        /// <summary>
        /// True when <paramref name="kind"/> is an equippable Accessory kind
        /// ("Item" or "Clothes") per the docs.
        /// </summary>
        public static bool IsEquippableKind(string? kind) =>
            string.Equals(kind, "Item", StringComparison.OrdinalIgnoreCase)
            || string.Equals(kind, "Clothes", StringComparison.OrdinalIgnoreCase);
    }
}
