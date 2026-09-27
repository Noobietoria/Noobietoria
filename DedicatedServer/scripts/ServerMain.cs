using System.Collections.Generic;
using System.Linq;
using Godot;
using Noobietoria.Shared;

namespace Noobietoria.DedicatedServer;

/// <summary>
/// Headless transport node: validates the handshake, relays chat and hands
/// the roster to newcomers. The root node must stay named "Main" — clients
/// reach it via the node path /root/Main.
///
/// Roster state (capacity, duplicate names, join order) lives in the
/// <see cref="ServerDedicatedServer"/> domain layer, which also owns the
/// authoritative Instance tree.
/// </summary>
public partial class ServerMain : Node
{
    private readonly Dictionary<int, string> _players = new();
    private ServerDedicatedServer _server = null!;
    private int _port = Net.DefaultPort;
    private Node3D _playersRoot = null!;

    public override void _Ready()
    {
        ParseArgs();
        _playersRoot = GetNode<Node3D>("Players");

        _server = new ServerDedicatedServer(Net.MaxPlayers);
        _server.Start();

        var peer = new ENetMultiplayerPeer();
        Error error = peer.CreateServer(_port, Net.MaxPlayers);
        if (error != Error.Ok)
        {
            GD.PrintErr($"[server] Could not listen on port {_port}: {error}");
            GetTree().Quit((int)error);
            return;
        }

        Multiplayer.MultiplayerPeer = peer;
        Multiplayer.PeerConnected += OnPeerConnected;
        Multiplayer.PeerDisconnected += OnPeerDisconnected;

        GD.Print($"[server] Noobietoria dedicated server listening on 0.0.0.0:{_port} (max {_server.MaxPlayers} players).");
        GD.Print("[server] Stop with Ctrl+C. Pass a different port with: -- --port 3000");
    }

    public override void _ExitTree()
    {
        if (_server is not null && _server.IsRunning)
            _server.Stop();
    }

    private void ParseArgs()
    {
        string[] args = OS.GetCmdlineUserArgs();
        for (int i = 0; i + 1 < args.Length; i++)
        {
            if (args[i] == "--port" && int.TryParse(args[i + 1], out int port) && port is >= 1 and <= 65535)
                _port = port;
        }
    }

    private void OnPeerConnected(long peerId)
    {
        GD.Print($"[server] Peer {peerId} connected, waiting for handshake.");
    }

    private void OnPeerDisconnected(long peerId)
    {
        if (_players.TryGetValue((int)peerId, out string? name))
        {
            _players.Remove((int)peerId);
            _server.Leave(name);
            GD.Print($"[server] {name} (#{peerId}) left. Players online: {_server.PlayerCount}");
            _playersRoot.GetNodeOrNull($"Player-{peerId}")?.QueueFree();
            Rpc(RpcMethod.PlayerLeft, (int)peerId, name);
            Rpc(RpcMethod.ReceiveChat, 0, "server", $"{name} left the game.");
        }
        else
        {
            GD.Print($"[server] Peer {peerId} dropped before handshake.");
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
        GD.Print($"[server] {name} (#{sender}) joined. Players online: {_server.PlayerCount}");

        // Mirror the client's node layout so avatar state RPCs
        // (/root/Main/Players/Player-N) have a matching path here.
        _playersRoot.AddChild(new PlayerRelay { Name = $"Player-{sender}" });

        RpcId(sender, RpcMethod.HandshakeAccepted, "Welcome to Noobietoria!",
            _players.Keys.ToArray(), _players.Values.ToArray());
        Rpc(RpcMethod.PlayerJoined, sender, name);
    }

    private void Reject(int peerId, string log, string reason)
    {
        GD.Print($"[server] {log}");
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

        GD.Print($"[chat] {name}: {text}");
        Rpc(RpcMethod.ReceiveChat, sender, name, text);
    }

    // ---- Stubs of the server-to-client RPCs. Their bodies only ever run on
    // clients, but Godot requires an [Rpc] config on the sending side too. ----

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
