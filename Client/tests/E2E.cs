using System.Collections.Generic;
using Godot;
using Noobietoria.Shared;

namespace Noobietoria.Client.Tests;

/// <summary>
/// Headless end-to-end test for the networking stack. Run the dedicated
/// server, then two instances of this scene (role "a" then role "b"):
///
///   godot --headless --path DedicatedServer -- --port 24599
///   E2E_ROLE=a E2E_PORT=24599 godot --headless --path Client res://tests/E2E.tscn
///   E2E_ROLE=b E2E_PORT=24599 godot --headless --path Client res://tests/E2E.tscn
///
/// Exit code 0 = pass. The scene root must stay named "Main" so RPCs match
/// the paths used by the real game and the dedicated server.
/// </summary>
public partial class E2E : Node
{
    private string _role = "a";
    private string _name = "Tester";
    private readonly List<int> _roster = new();

    private int _handshakes;
    private int _chatReceived;
    private bool _sawOtherJoin;
    private bool _sawOtherLeave;
    private bool _finished;

    public override void _Ready()
    {
        _role = OS.GetEnvironment("E2E_ROLE") is { Length: > 0 } r ? r : "a";
        _name = _role == "a" ? "Alpha" : "Beta";
        int port = int.TryParse(OS.GetEnvironment("E2E_PORT"), out int p) ? p : 24599;

        var peer = new ENetMultiplayerPeer();
        Error error = peer.CreateClient("127.0.0.1", port);
        if (error != Error.Ok)
        {
            GD.PrintErr($"E2E: could not start client: {error}");
            GetTree().Quit(2);
            return;
        }

        Multiplayer.MultiplayerPeer = peer;
        Multiplayer.ConnectedToServer += () => RpcId(1, RpcMethod.SubmitHandshake, _name, Net.ProtocolTag, "");

        GetTree().CreateTimer(10.0).Timeout += () =>
        {
            GD.PrintErr("E2E: TIMEOUT");
            PrintSummary();
            GetTree().Quit(3);
        };

        GD.Print($"E2E[{_name}]: connecting to 127.0.0.1:{port}");
    }

    // Stubs of client-to-server RPCs — required on the sending side, no-op here.
    [Rpc(MultiplayerApi.RpcMode.AnyPeer, CallLocal = false)]
    private void SubmitHandshake(string playerName, string protocolTag, string ticket) { }

    [Rpc(MultiplayerApi.RpcMode.AnyPeer, CallLocal = false)]
    private void SendChat(string text) { }

    [Rpc(CallLocal = false)]
    private void HandshakeAccepted(string motd, int[] ids, string[] names)
    {
        _handshakes++;
        _roster.AddRange(ids);
        GD.Print($"E2E[{_name}]: handshake ok, roster=[{string.Join(",", ids)}] names=[{string.Join(",", names)}]");

        if (_role == "b")
            RpcId(1, RpcMethod.SendChat, "hello from Beta");
        else if (_role == "solo")
            Finish(_roster.Count == 1 && _roster[0] == Multiplayer.GetUniqueId() ? 0 : 5);
    }

    [Rpc(CallLocal = false)]
    private void HandshakeRejected(string reason)
    {
        GD.PrintErr($"E2E[{_name}]: rejected: {reason}");
        GetTree().Quit(4);
    }

    [Rpc(CallLocal = false)]
    private void PlayerJoined(int peerId, string displayName)
    {
        if (peerId == Multiplayer.GetUniqueId())
            return;

        _sawOtherJoin = true;
        GD.Print($"E2E[{_name}]: saw {displayName} #{peerId} join");

        if (_role == "a")
            RpcId(1, RpcMethod.SendChat, "welcome Beta!");
    }

    [Rpc(CallLocal = false)]
    private void PlayerLeft(int peerId, string displayName)
    {
        _sawOtherLeave = true;
        GD.Print($"E2E[{_name}]: saw {displayName} #{peerId} leave");
    }

    [Rpc(CallLocal = false)]
    private void ReceiveChat(int senderId, string senderName, string text)
    {
        if (senderId == 0)
            return; // system messages

        _chatReceived++;
        GD.Print($"E2E[{_name}]: chat from {senderName}: {text}");

        if (_role == "b" && senderName == "Alpha")
            Finish(0);
        else if (_role == "a" && senderName == "Beta")
            GetTree().CreateTimer(1.5).Timeout += () => Finish(0);
    }

    private void Finish(int code)
    {
        if (_finished)
            return;
        _finished = true;
        PrintSummary();
        GetTree().Quit(code);
    }

    private void PrintSummary()
    {
        GD.Print($"E2E[{_name}] SUMMARY handshakes={_handshakes} rosterCount={_roster.Count} " +
                 $"sawJoin={_sawOtherJoin} sawLeave={_sawOtherLeave} chatReceived={_chatReceived}");
    }
}
