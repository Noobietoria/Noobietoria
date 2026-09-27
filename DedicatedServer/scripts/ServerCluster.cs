using System;
using System.Collections.Generic;
using System.Text.Json;
using Godot;
using Noobietoria.Shared;

namespace Noobietoria.DedicatedServer;

/// <summary>
/// Supervisor for one server process hosting MANY places (game worlds) at
/// once — each place gets its own ENet port, SceneMultiplayer and
/// authoritative ServerDedicatedServer. Places can also be added/removed
/// at runtime through the Luau-facing ServerClusterService later.
///
/// Launch options (user args after "--"):
///   --place &lt;name&gt;:&lt;port&gt;    host one place (repeatable)
///   --places &lt;file.json&gt;       bulk host from a JSON file
///       format: [ { "name": "lobby", "port": 24565 }, ... ]
///   --port &lt;port&gt;              shorthand for a single unnamed place
///   With no arguments: one place named "default" on Net.DefaultPort.
///
/// See: https://noobietoria.github.io/Docs/en/api/#instanceservice
/// </summary>
public partial class ServerCluster : Node
{
    private sealed record PlaceSpec(string Name, int Port);

    private sealed class PlaceFileEntry
    {
        public string Name { get; set; } = "";
        public int Port { get; set; }
    }

    private int _placesStarted;

    public override void _Ready()
    {
        foreach (var place in ParsePlaces())
            StartPlace(place.Name, place.Port);

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
                    places.Add(new PlaceSpec("default", port));
                    i++;
                    break;
            }
        }

        if (places.Count == 0)
            places.Add(new PlaceSpec("default", Net.DefaultPort));
        return places;
    }

    private static PlaceSpec? ParsePlaceSpec(string argument)
    {
        // "<name>:<port>" — e.g. "lobby:24565"
        int separator = argument.LastIndexOf(':');
        if (separator <= 0 || separator == argument.Length - 1
            || !int.TryParse(argument[(separator + 1)..], out int port) || port is < 1 or > 65535)
        {
            GD.PrintErr($"[cluster] Ignoring malformed --place '{argument}' (expected \"<name>:<port>\").");
            return null;
        }
        return new PlaceSpec(argument[..separator], port);
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
            var entries = JsonSerializer.Deserialize<List<PlaceFileEntry>>(json,
                new JsonSerializerOptions { PropertyNameCaseInsensitive = true })
                ?? new List<PlaceFileEntry>();
            foreach (var entry in entries)
            {
                if (string.IsNullOrWhiteSpace(entry.Name))
                    throw new InvalidOperationException("place entry is missing \"name\".");
                if (entry.Port is < 1 or > 65535)
                    throw new InvalidOperationException($"place '{entry.Name}' has an invalid port.");
                places.Add(new PlaceSpec(entry.Name.Trim(), entry.Port));
            }
        }
        catch (Exception e)
        {
            GD.PrintErr($"[cluster] Could not load places file '{path}': {e.Message}");
        }
        return places;
    }

    private void StartPlace(string placeName, int port)
    {
        var peer = new ENetMultiplayerPeer();
        Error error = peer.CreateServer(port, Net.MaxPlayers);
        if (error != Error.Ok)
        {
            GD.PrintErr($"[cluster] Place '{placeName}': could not listen on port {port}: {error}");
            return;
        }

        // The scene is a container: PlaceRoot/Main(PlaceServer)+Players.
        // RPC paths from clients are relative to the tree root ("Main",
        // "Main/Players/Player-N"), so the subtree root must be the
        // container and the PlaceServer must sit at child path "Main".
        string nodeName = $"Place-{placeName}";
        string placePath = GetPath() + "/" + nodeName;

        var placeRoot = GD.Load<PackedScene>("res://scenes/PlaceServer.tscn").Instantiate<Node>();
        placeRoot.Name = nodeName;
        var place = placeRoot.GetNode<PlaceServer>("Main");
        place.PlaceName = placeName;

        var api = new SceneMultiplayer { MultiplayerPeer = peer, RootPath = placePath };
        GetTree().SetMultiplayer(api, placePath);
        AddChild(placeRoot);

        _placesStarted++;
        GD.Print($"[cluster] Place '{placeName}' listening on 0.0.0.0:{port} ({_placesStarted} place(s) hosted).");
    }
}
