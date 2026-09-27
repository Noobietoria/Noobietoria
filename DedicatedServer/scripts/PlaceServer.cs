using System.Collections.Generic;
using System.Linq;
using Godot;
using Noobietoria.Shared;

namespace Noobietoria.DedicatedServer;

/// <summary>
/// One running place (game world) inside the server cluster. Each
/// PlaceServer owns its own SceneMultiplayer + ENetMultiplayerPeer
/// (attached by <see cref="ServerCluster"/> via SceneTree.SetMultiplayer),
/// so a single process hosts many places at once. The node layout mirrors
/// the client's /root/Main: PlaceServer("Main") + Players/Player-N stubs —
/// so avatar RPC paths resolve on every peer.
///
/// Roster state (capacity, duplicate names, join order) lives in the
/// <see cref="ServerDedicatedServer"/> domain layer, which also owns the
/// authoritative Instance tree — one per place.
/// </summary>
public partial class PlaceServer : Node
{
    private readonly Dictionary<int, string> _players = new();
    private ServerDedicatedServer _server = null!;
    private Node3D _playersRoot = null!;

    /// <summary>Logical name of the place (used for logging).</summary>
    public string PlaceName { get; set; } = "default";

    public override void _Ready()
    {
        _playersRoot = GetNode<Node3D>("Players");

        _server = new ServerDedicatedServer(Net.MaxPlayers);
        _server.Start();

        var peer = Multiplayer.MultiplayerPeer;
        if (!Multiplayer.IsServer())
        {
            GD.PrintErr($"[cluster:{PlaceName}] No server peer attached — place is not listening.");
            return;
        }

        Multiplayer.PeerConnected += OnPeerConnected;
        Multiplayer.PeerDisconnected += OnPeerDisconnected;

        GD.Print($"[cluster:{PlaceName}] hosting place (max {_server.MaxPlayers} players).");
    }

    public override void _ExitTree()
    {
        if (_server is not null && _server.IsRunning)
            _server.Stop();
    }

    private void OnPeerConnected(long peerId)
    {
        GD.Print($"[cluster:{PlaceName}] Peer {peerId} connected, waiting for handshake.");
    }

    private void OnPeerDisconnected(long peerId)
    {
        if (_players.TryGetValue((int)peerId, out string? name))
        {
            _players.Remove((int)peerId);
            _server.Leave(name);
            GD.Print($"[cluster:{PlaceName}] {name} (#{peerId}) left. Players online: {_server.PlayerCount}");
            _playersRoot.GetNodeOrNull($"Player-{peerId}")?.QueueFree();
            Rpc(RpcMethod.PlayerLeft, (int)peerId, name);
            Rpc(RpcMethod.ReceiveChat, 0, "server", $"{name} left the game.");
        }
        else
        {
            GD.Print($"[cluster:{PlaceName}] Peer {peerId} dropped before handshake.");
        }
    }

    [Rpc(MultiplayerApi.RpcMode.AnyPeer, CallLocal = false)]
    private void SubmitHandshake(string playerName, string protocolTag)
    {
        int sender = Multiplayer.GetRemoteSenderId();

        if (protocolTag != Net.ProtocolTag)
        {
            Reject(sender, $"Rejected #{sender}: protocol mismatch ('{protocolTag}' != '{Net.ProtocolTag}').",
                "Your client version does not match this server.");
            return;
        }

        if (_players.Count >= Net.MaxPlayers)
        {
            Reject(sender, $"Rejected #{sender}: server full.", "The server is full, try again later.");
            return;
        }

        string name = string.IsNullOrWhiteSpace(playerName) ? $"Player{sender}" : playerName.Trim();
        name = name.Replace("[", "").Replace("]", "");
        if (name.Length > 24)
            name = name[..24];

        // Usernames must be unique in the domain layer — de-duplicate by peer id.
        if (_server.IsOnline(name))
            name = $"{name[..Math.Min(name.Length, 19)]}#{sender % 1000:000}";

        try
        {
            _server.Join(name);
        }
        catch (InvalidOperationException e)
        {
            Reject(sender, $"Rejected #{sender}: {e.Message}", "The server is full, try again later.");
            return;
        }

        _players[sender] = name;
        GD.Print($"[cluster:{PlaceName}] {name} (#{sender}) joined. Players online: {_server.PlayerCount}");

        // Mirror the client's node layout so avatar state RPCs
        // (Players/Player-N relative to the place root) have a matching path.
        _playersRoot.AddChild(new PlayerRelay { Name = $"Player-{sender}" });

        RpcId(sender, RpcMethod.HandshakeAccepted, $"Welcome to {PlaceName}!",
            _players.Keys.ToArray(), _players.Values.ToArray());
        Rpc(RpcMethod.PlayerJoined, sender, name);
    }

    private void Reject(int peerId, string log, string reason)
    {
        GD.Print($"[cluster:{PlaceName}] {log}");
        RpcId(peerId, RpcMethod.HandshakeRejected, reason);
        Multiplayer.MultiplayerPeer.DisconnectPeer(peerId);
    }

    [Rpc(MultiplayerApi.RpcMode.AnyPeer, CallLocal = false)]
    private void SendChat(string text)
    {
        int sender = Multiplayer.GetRemoteSenderId();
        if (!_players.TryGetValue(sender, out string? name))
            return;

        text = text.Trim();
        if (text.Length == 0)
            return;
        if (text.Length > 256)
            text = text[..256];

        GD.Print($"[cluster:{PlaceName}][chat] {name}: {text}");
        Rpc(RpcMethod.ReceiveChat, sender, name, text);
    }

    // ---- Stubs of the server-to-client RPCs. Their bodies only ever run on
    // clients, but Godot requires an [Rpc] config on the sender too. ----

    [Rpc(CallLocal = false)]
    private void HandshakeAccepted(string motd, int[] ids, string[] names) { }

    [Rpc(CallLocal = false)]
    private void HandshakeRejected(string reason) { }

    [Rpc(CallLocal = false)]
    private void PlayerJoined(int peerId, string displayName) { }

    [Rpc(CallLocal = false)]
    private void PlayerLeft(int peerId, string displayName) { }

    [Rpc(CallLocal = false)]
    private void ReceiveChat(int senderId, string senderName, string text) { }
}
