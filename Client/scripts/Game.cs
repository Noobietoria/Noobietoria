using System.Collections.Generic;
using Godot;
using Noobietoria.Shared;

namespace Noobietoria.Client;

/// <summary>
/// The in-game scene: spawns an avatar for every player, keeps the roster UI
/// up to date and relays chat through the server. The root node must stay
/// named "Main" — RPCs are matched by node path against the server's
/// /root/Main.
/// </summary>
public partial class Game : Node3D
{
    /// <summary>True while the chat input owns keyboard focus; avatars stop moving.</summary>
    public static bool ChatFocused;

    private readonly Dictionary<int, string> _roster = new();

    private Node3D _playersRoot = null!;
    private PackedScene _playerScene = null!;
    private RichTextLabel _chatLog = null!;
    private LineEdit _chatInput = null!;
    private Label _rosterLabel = null!;
    private Camera3D _camera = null!;

    public override void _Ready()
    {
        _playersRoot = GetNode<Node3D>("%Players");
        _playerScene = GD.Load<PackedScene>("res://scenes/Player.tscn");
        _chatLog = GetNode<RichTextLabel>("%ChatLog");
        _chatInput = GetNode<LineEdit>("%ChatInput");
        _rosterLabel = GetNode<Label>("%RosterLabel");
        _camera = GetNode<Camera3D>("%Cam");

        GetNode<DirectionalLight3D>("%Sun").RotationDegrees = new Vector3(-50f, 25f, 0f);

        _chatInput.TextSubmitted += OnChatSubmitted;
        _chatInput.FocusEntered += () => ChatFocused = true;
        _chatInput.FocusExited += () => ChatFocused = false;
        GetNode<Button>("%SendButton").Pressed += () => OnChatSubmitted(_chatInput.Text);

        Multiplayer.ServerDisconnected += OnServerDisconnected;

        SpawnPlayer(Multiplayer.GetUniqueId(), ClientState.UserName);
        _roster[Multiplayer.GetUniqueId()] = ClientState.UserName;

        // Announce ourselves; the server answers with the full roster.
        if (HasLivePeer)
        {
            RpcId(1, RpcMethod.SubmitHandshake, ClientState.UserName, Net.ProtocolTag);
            AppendSystemMessage($"Connecting as {ClientState.UserName}…");
        }
        else
        {
            // Scene opened standalone (no menu connection) — stay in offline mode.
            AppendSystemMessage("Offline — start the game from the main menu to connect.");
        }
    }

    public override void _ExitTree()
    {
        Multiplayer.ServerDisconnected -= OnServerDisconnected;
        ChatFocused = false;
    }

    public override void _Process(double delta)
    {
        // Soft follow camera behind our own avatar.
        Node3D? self = _playersRoot.GetNodeOrNull<Node3D>($"Player-{Multiplayer.GetUniqueId()}");
        if (self is null)
            return;

        float weight = Mathf.Min(1.0f, (float)delta * 6.0f);
        _camera.Position = _camera.Position.Lerp(self.Position + new Vector3(0f, 7.5f, 9f), weight);
        _camera.LookAt(self.Position + Vector3.Up);
    }

    private void SpawnPlayer(int peerId, string displayName)
    {
        if (_playersRoot.GetNodeOrNull($"Player-{peerId}") is not null)
            return;

        var player = _playerScene.Instantiate<Player>();
        player.PeerId = peerId;
        player.DisplayName = displayName;
        player.Name = $"Player-{peerId}";
        _playersRoot.AddChild(player);
    }

    private void UpdateRosterLabel()
    {
        _rosterLabel.Text = $"Players ({_roster.Count})\n" + string.Join("\n", _roster.Values);
    }

    private void AppendSystemMessage(string message)
        => _chatLog.AppendText($"[color=#8a8f98]{message}[/color]\n");

    /// <summary>True when a real (non-offline) multiplayer peer is set.</summary>
    private bool HasLivePeer => Multiplayer.MultiplayerPeer is not (null or OfflineMultiplayerPeer);

    private void OnChatSubmitted(string text)
    {
        text = text.Trim();
        if (text.Length == 0)
            return;
        if (!HasLivePeer)
        {
            AppendSystemMessage("Not connected to a server.");
            return;
        }

        _chatInput.Clear();
        RpcId(1, RpcMethod.SendChat, text);
    }

    private void OnServerDisconnected()
    {
        Multiplayer.MultiplayerPeer = new OfflineMultiplayerPeer();
        ClientState.LastError = "Disconnected from the server.";
        GetTree().ChangeSceneToFile("res://scenes/MainMenu.tscn");
    }

    // ---- Stubs of the client-to-server RPCs. Their bodies only ever run on
    // the server, but Godot requires an [Rpc] config on the sending side too. ----

    [Rpc(MultiplayerApi.RpcMode.AnyPeer, CallLocal = false)]
    private void SubmitHandshake(string playerName, string protocolTag) { }

    [Rpc(MultiplayerApi.RpcMode.AnyPeer, CallLocal = false)]
    private void SendChat(string text) { }

    // ---- RPCs invoked by the dedicated server on /root/Main ----

    [Rpc(CallLocal = false)]
    private void HandshakeAccepted(string motd, int[] ids, string[] names)
    {
        if (Multiplayer.GetRemoteSenderId() != 1)
            return;

        int selfId = Multiplayer.GetUniqueId();
        for (int i = 0; i < ids.Length; i++)
        {
            _roster[ids[i]] = names[i];
            if (ids[i] != selfId)
                SpawnPlayer(ids[i], names[i]);
        }

        UpdateRosterLabel();
        AppendSystemMessage(motd);
    }

    [Rpc(CallLocal = false)]
    private void HandshakeRejected(string reason)
    {
        Multiplayer.MultiplayerPeer = new OfflineMultiplayerPeer();
        ClientState.LastError = reason;
        GetTree().ChangeSceneToFile("res://scenes/MainMenu.tscn");
    }

    [Rpc(CallLocal = false)]
    private void PlayerJoined(int peerId, string displayName)
    {
        if (peerId == Multiplayer.GetUniqueId() || _roster.ContainsKey(peerId))
            return;

        _roster[peerId] = displayName;
        SpawnPlayer(peerId, displayName);
        AppendSystemMessage($"{displayName} joined the game.");
        UpdateRosterLabel();
    }

    [Rpc(CallLocal = false)]
    private void PlayerLeft(int peerId, string displayName)
    {
        _roster.Remove(peerId);
        _playersRoot.GetNodeOrNull($"Player-{peerId}")?.QueueFree();
        AppendSystemMessage($"{displayName} left the game.");
        UpdateRosterLabel();
    }

    [Rpc(CallLocal = false)]
    private void ReceiveChat(int senderId, string senderName, string text)
    {
        if (senderId == 0)
            AppendSystemMessage(text);
        else
            _chatLog.AppendText($"[color=#9cd6ff]{senderName}[/color]: {text}\n");
    }
}
