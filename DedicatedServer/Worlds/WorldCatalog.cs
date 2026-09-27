using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using Noobietoria.Platform;

namespace Noobietoria.DedicatedServer;

/// <summary>
/// Loads the world definitions a place can be attached to. A world is the
/// per-place content identity: name, description and spawn layout. v0 ships
/// JSON definitions in DedicatedServer/worlds; with the Luau runtime these
/// become full scriptable worlds published per game.
///
/// Files use camelCase JSON:
///   { "id": "grasslands", "name": "Grasslands Social",
///     "description": "...", "spawn": {"x":0,"y":1,"z":0}, "spawnRadius": 6 }
/// </summary>
public class WorldCatalog
{
    private sealed class SpawnDto
    {
        public double X { get; set; }
        public double Y { get; set; }
        public double Z { get; set; }
    }

    private sealed class WorldDto
    {
        public string Id { get; set; } = "";
        public string Name { get; set; } = "";
        public string Description { get; set; } = "";
        public SpawnDto? Spawn { get; set; }
        public double SpawnRadius { get; set; } = 6;
    }

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNameCaseInsensitive = true,
    };

    private readonly Dictionary<string, WorldDefinition> _worlds = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>Worlds loaded in this catalog.</summary>
    public IReadOnlyList<WorldDefinition> Worlds => _worlds.Values.ToArray();

    /// <summary>Loads every *.json world definition in a directory.</summary>
    public static WorldCatalog LoadFromDirectory(string directory)
    {
        var catalog = new WorldCatalog();
        if (!Directory.Exists(directory))
            return catalog;

        foreach (string file in Directory.GetFiles(directory, "*.json"))
        {
            try
            {
                var dto = JsonSerializer.Deserialize<WorldDto>(File.ReadAllText(file), JsonOptions);
                if (dto == null || string.IsNullOrWhiteSpace(dto.Id))
                    throw new InvalidOperationException("world definition is missing \"id\".");

                catalog._worlds[dto.Id] = new WorldDefinition(
                    dto.Id, dto.Name, dto.Description,
                    dto.Spawn?.X ?? 0, dto.Spawn?.Y ?? 1, dto.Spawn?.Z ?? 0, dto.SpawnRadius);
            }
            catch (Exception e)
            {
                throw new InvalidOperationException($"Invalid world definition '{file}': {e.Message}", e);
            }
        }
        return catalog;
    }

    /// <summary>
    /// Resolves a world id; unknown ids fall back to a synthetic definition
    /// so a place is always playable.
    /// </summary>
    public WorldDefinition GetOrFallback(string worldId)
    {
        if (_worlds.TryGetValue(worldId, out var world))
            return world;
        return new WorldDefinition(worldId, worldId, string.Empty, 0, 1, 0, 6);
    }

    /// <summary>Whether the catalog knows this world.</summary>
    public bool Contains(string worldId) => _worlds.ContainsKey(worldId);
}
