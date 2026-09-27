using System;
using System.Collections.Generic;
using System.Text.Json;
using Godot;
using Noobietoria.Shared;

namespace Noobietoria.DedicatedServer;

/// <summary>
/// Supervisor for one server process hosting MANY places (game worlds) at
/// once — each place is attached to a World, gets its own ENet port,
/// SceneMultiplayer and authoritative ServerDedicatedServer. Places can
/// also be added/removed at runtime through the Luau-facing
/// ServerClusterService later.
///
/// Launch options (user args after "--"):
///   --place &lt;name&gt;:&lt;port&gt;[:&lt;worldId&gt;]   host one place (repeatable)
///   --places &lt;file.json&gt;                bulk host from a JSON file
///       format: [ { "name": "lobby", "port": 24565, "worldId": "grasslands" }, ... ]
///   --port &lt;port&gt;                       shorthand for a single unnamed place
///   --platform-secret &lt;hex&gt;             require HMAC join tickets signed
///                                       with this fleet secret
///   With no arguments: one place named "default" on Net.DefaultPort.
///
/// The place/world registry normally comes from the platform backend
/// (closed source, Cloudflare stack) — the CLI/JSON shapes are the same
/// contract the backend provisions.
/// </summary>
public partial class ServerCluster : Node
{
    private sealed record PlaceSpec(string Name, int Port, string WorldId);

    private sealed class PlaceFileEntry
    {
        public string Name { get; set; } = "";
        public int Port { get; set; }
        public string WorldId { get; set; } = "grasslands";
    }

    private static readonly JsonSerializerOptions PlacesJsonOptions = new()
    {
        PropertyNameCaseInsensitive = true,
    };

    private int _placesStarted;

    public override void _Ready()
    {
        byte[]? ticketSecret = ParseTicketSecret();
        WorldCatalog catalog = LoadWorlds();

        foreach (var place in ParsePlaces())
            StartPlace(place, catalog, ticketSecret);

        if (_placesStarted == 0)
        {
            GD.PrintErr("[cluster] No place could be started — check the log for port conflicts.");
            GetTree().Quit((int)Error.CantCreate);
        }
    }

    private List<PlaceSpec> ParsePlaces()
    {
        var places = new List<PlaceSpec>();
        string[] args = OS.GetCmdlineUserArgs();

        for (int i = 0; i < args.Length; i++)
        {
            switch (args[i])
            {
                case "--place" when i + 1 < args.Length:
                {
                    var spec = ParsePlaceSpec(args[++i]);
                    if (spec != null)
                        places.Add(spec);
                    break;
                }
                case "--places" when i + 1 < args.Length:
                    places.AddRange(LoadPlacesFile(args[++i]));
                    break;
                case "--port" when i + 1 < args.Length
                    && int.TryParse(args[i + 1], out int port) && port is >= 1 and <= 65535:
                    places.Add(new PlaceSpec("default", port, "grasslands"));
                    i++;
                    break;
            }
        }

        if (places.Count == 0)
            places.Add(new PlaceSpec("default", Net.DefaultPort, "grasslands"));
        return places;
    }

    private byte[]? ParseTicketSecret()
    {
        string[] args = OS.GetCmdlineUserArgs();
        for (int i = 0; i + 1 < args.Length; i++)
        {
            if (args[i] == "--platform-secret")
            {
                try
                {
                    return Convert.FromHexString(args[i + 1].Trim());
                }
                catch (FormatException)
                {
                    GD.PrintErr("[cluster] --platform-secret must be a hex string — tickets stay OFF.");
                    return null;
                }
            }
        }
        return null;
    }

    private WorldCatalog LoadWorlds()
    {
        string worldsDir = ProjectSettings.GlobalizePath("res://worlds");
        var catalog = WorldCatalog.LoadFromDirectory(worldsDir);
        if (catalog.Worlds.Count == 0)
            GD.PrintErr($"[cluster] No world definitions found in '{worldsDir}' — places fall back to synthetic worlds.");
        return catalog;
    }

    private static PlaceSpec? ParsePlaceSpec(string argument)
    {
        // "<name>:<port>[:<worldId>]" — e.g. "lobby:24565" or "lobby:24565:grasslands"
        string[] segments = argument.Split(':');
        if (segments.Length < 2
            || !int.TryParse(segments[1], out int port) || port is < 1 or > 65535)
        {
            GD.PrintErr($"[cluster] Ignoring malformed --place '{argument}' (expected \"<name>:<port>[:<worldId>]\").");
            return null;
        }
        string worldId = segments.Length > 2 && segments[2].Length > 0 ? segments[2] : "grasslands";
        return new PlaceSpec(segments[0], port, worldId);
    }

    private List<PlaceSpec> LoadPlacesFile(string path)
    {
        var places = new List<PlaceSpec>();
        try
        {
            var file = Godot.FileAccess.Open(path, Godot.FileAccess.ModeFlags.Read);
            if (file == null)
            {
                GD.PrintErr($"[cluster] Could not open places file '{path}'.");
                return places;
            }

            string json = file.GetAsText();
            var entries = JsonSerializer.Deserialize<List<PlaceFileEntry>>(json, PlacesJsonOptions)
                ?? new List<PlaceFileEntry>();
            foreach (var entry in entries)
            {
                if (string.IsNullOrWhiteSpace(entry.Name))
                    throw new InvalidOperationException("place entry is missing \"name\".");
                if (entry.Port is < 1 or > 65535)
                    throw new InvalidOperationException($"place '{entry.Name}' has an invalid port.");
                places.Add(new PlaceSpec(entry.Name.Trim(), entry.Port,
                    string.IsNullOrWhiteSpace(entry.WorldId) ? "grasslands" : entry.WorldId.Trim()));
            }
        }
        catch (Exception e)
        {
            GD.PrintErr($"[cluster] Could not load places file '{path}': {e.Message}");
        }
        return places;
    }

    private void StartPlace(PlaceSpec spec, WorldCatalog catalog, byte[]? ticketSecret)
    {
        var peer = new ENetMultiplayerPeer();
        Error error = peer.CreateServer(spec.Port, Net.MaxPlayers);
        if (error != Error.Ok)
        {
            GD.PrintErr($"[cluster] Place '{spec.Name}': could not listen on port {spec.Port}: {error}");
            return;
        }

        // The scene is a container: PlaceRoot/Main(PlaceServer)+Players.
        // RPC paths from clients are relative to the tree root ("Main",
        // "Main/Players/Player-N"), so the subtree root must be the
        // container and the PlaceServer must sit at child path "Main".
        string nodeName = $"Place-{spec.Name}";
        string placePath = GetPath() + "/" + nodeName;

        var placeRoot = GD.Load<PackedScene>("res://scenes/PlaceServer.tscn").Instantiate<Node>();
        placeRoot.Name = nodeName;
        var place = placeRoot.GetNode<PlaceServer>("Main");
        place.PlaceName = spec.Name;
        place.WorldId = spec.WorldId;
        place.Catalog = catalog;
        place.TicketSecret = ticketSecret;

        var api = new SceneMultiplayer { MultiplayerPeer = peer, RootPath = placePath };
        GetTree().SetMultiplayer(api, placePath);
        AddChild(placeRoot);

        _placesStarted++;
        GD.Print($"[cluster] Place '{spec.Name}' (world '{spec.WorldId}') listening on 0.0.0.0:{spec.Port} ({_placesStarted} place(s) hosted).");
    }
}
