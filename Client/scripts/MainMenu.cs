using System;
using System.Collections.Generic;
using System.Linq;
using Godot;
using Noobietoria.Platform;

namespace Noobietoria.Client;

/// <summary>
/// Main menu: the platform portal (log in, browse games, play) plus a
/// direct-connect fallback for development.
/// </summary>
public partial class MainMenu : Control
{
    private LineEdit _nameInput = null!;
    private LineEdit _addressInput = null!;
    private LineEdit _portInput = null!;
    private Button _connectButton = null!;
    private Label _statusLabel = null!;

    private LineEdit _platformUserInput = null!;
    private LineEdit _platformPasswordInput = null!;
    private LineEdit _platformUrlInput = null!;
    private Button _loginButton = null!;
    private ItemList _gamesList = null!;
    private Button _playButton = null!;
    private Label _platformStatusLabel = null!;

    private PlatformClient? _platform;
    private PlatformAccount? _account;
    private IReadOnlyList<GameListing> _games = Array.Empty<GameListing>();

    public override void _Ready()
    {
        _nameInput = GetNode<LineEdit>("%NameInput");
        _addressInput = GetNode<LineEdit>("%AddressInput");
        _portInput = GetNode<LineEdit>("%PortInput");
        _connectButton = GetNode<Button>("%ConnectButton");
        _statusLabel = GetNode<Label>("%StatusLabel");

        _platformUserInput = GetNode<LineEdit>("%PlatformUserInput");
        _platformPasswordInput = GetNode<LineEdit>("%PlatformPasswordInput");
        _platformUrlInput = GetNode<LineEdit>("%PlatformUrlInput");
        _loginButton = GetNode<Button>("%LoginButton");
        _gamesList = GetNode<ItemList>("%GamesList");
        _playButton = GetNode<Button>("%PlayButton");
        _platformStatusLabel = GetNode<Label>("%PlatformStatusLabel");

        _nameInput.Text = ClientState.UserName;
        _addressInput.Text = ClientState.Address;
        _portInput.Text = ClientState.Port.ToString();

        _connectButton.Pressed += OnConnectPressed;
        Multiplayer.ConnectedToServer += OnConnectedToServer;
        Multiplayer.ConnectionFailed += OnConnectionFailed;

        _loginButton.Pressed += OnLoginPressed;
        _playButton.Pressed += OnPlayPressed;

        if (ClientState.LastError is { } error)
        {
            _statusLabel.Text = error;
            ClientState.LastError = null;
        }
    }

    public override void _ExitTree()
    {
        _connectButton.Pressed -= OnConnectPressed;
        Multiplayer.ConnectedToServer -= OnConnectedToServer;
        Multiplayer.ConnectionFailed -= OnConnectionFailed;
        _loginButton.Pressed -= OnLoginPressed;
        _playButton.Pressed -= OnPlayPressed;
    }

    // ---- platform portal ----

    private async void OnLoginPressed()
    {
        string username = _platformUserInput.Text.Trim();
        string password = _platformPasswordInput.Text;
        if (username.Length == 0)
        {
            _platformStatusLabel.Text = "Pick an account name first.";
            return;
        }

        _loginButton.Disabled = true;
        _playButton.Disabled = true;
        _platformStatusLabel.Text = "Logging in…";

        try
        {
            _platform = new PlatformClient(_platformUrlInput.Text.Trim());
            _account = await _platform.LoginAsync(username, password);
            _games = await _platform.GetGamesAsync(_account.Token);

            _gamesList.Clear();
            foreach (var game in _games)
                _gamesList.AddItem($"{game.Title}  ·  {game.PlayersOnline} online");

            _playButton.Disabled = _games.Count == 0;
            _platformStatusLabel.Text = _platform.OfflineMode
                ? $"Logged in as {_account.Username} (offline dev platform)."
                : $"Logged in as {_account.Username}.";
        }
        catch (Exception e)
        {
            _platformStatusLabel.Text = $"Login failed: {e.Message}";
        }
        finally
        {
            _loginButton.Disabled = false;
        }
    }

    private async void OnPlayPressed()
    {
        if (_platform == null || _account == null || _games.Count == 0)
            return;

        int selected = _gamesList.GetSelectedItems().FirstOrDefault();
        if (_gamesList.GetSelectedItems().Length == 0)
        {
            _platformStatusLabel.Text = "Select a game to play.";
            return;
        }
        var game = _games[selected];

        _playButton.Disabled = true;
        _platformStatusLabel.Text = $"Joining {game.Title}…";

        try
        {
            var join = await _platform.RequestJoinAsync(game.GameId, _account.Token);

            ClientState.UserName = string.IsNullOrWhiteSpace(_nameInput.Text.Trim())
                ? _account.Username
                : _nameInput.Text.Trim();
            ClientState.Address = join.Host;
            ClientState.Port = join.Port;
            ClientState.Ticket = join.Ticket;
            ClientState.GameTitle = game.Title;

            GetTree().ChangeSceneToFile("res://scenes/Main.tscn");
        }
        catch (Exception e)
        {
            _playButton.Disabled = false;
            _platformStatusLabel.Text = $"Join failed: {e.Message}";
        }
    }

    // ---- direct connect (dev) ----

    private void OnConnectPressed()
    {
        string name = _nameInput.Text.Trim();
        if (name.Length == 0)
        {
            _statusLabel.Text = "Pick a player name first.";
            return;
        }

        string address = _addressInput.Text.Trim();
        if (address.Length == 0)
        {
            _statusLabel.Text = "Enter a server address.";
            return;
        }

        if (!int.TryParse(_portInput.Text.Trim(), out int port) || port is < 1 or > 65535)
        {
            _statusLabel.Text = "Port must be between 1 and 65535.";
            return;
        }

        ClientState.UserName = name;
        ClientState.Address = address;
        ClientState.Port = port;
        ClientState.Ticket = string.Empty;

        _statusLabel.Text = $"Connecting to {address}:{port}…";
        _connectButton.Disabled = true;

        var peer = new ENetMultiplayerPeer();
        Error error = peer.CreateClient(address, port);
        if (error != Error.Ok)
        {
            _connectButton.Disabled = false;
            _statusLabel.Text = $"Could not start the client: {error}";
            return;
        }

        Multiplayer.MultiplayerPeer = peer;
    }

    private void OnConnectedToServer()
    {
        ClientState.LastError = null;
        GetTree().ChangeSceneToFile("res://scenes/Main.tscn");
    }

    private void OnConnectionFailed()
    {
        Multiplayer.MultiplayerPeer = new OfflineMultiplayerPeer();
        _connectButton.Disabled = false;
        _statusLabel.Text = "Could not connect. Check the address and port.";
    }
}
