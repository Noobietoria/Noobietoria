using Godot;

namespace Noobietoria.Client;

/// <summary>Main menu: collects name/address/port and opens the ENet connection.</summary>
public partial class MainMenu : Control
{
    private LineEdit _nameInput = null!;
    private LineEdit _addressInput = null!;
    private LineEdit _portInput = null!;
    private Button _connectButton = null!;
    private Label _statusLabel = null!;

    public override void _Ready()
    {
        _nameInput = GetNode<LineEdit>("%NameInput");
        _addressInput = GetNode<LineEdit>("%AddressInput");
        _portInput = GetNode<LineEdit>("%PortInput");
        _connectButton = GetNode<Button>("%ConnectButton");
        _statusLabel = GetNode<Label>("%StatusLabel");

        _nameInput.Text = ClientState.UserName;
        _addressInput.Text = ClientState.Address;
        _portInput.Text = ClientState.Port.ToString();

        _connectButton.Pressed += OnConnectPressed;
        Multiplayer.ConnectedToServer += OnConnectedToServer;
        Multiplayer.ConnectionFailed += OnConnectionFailed;

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
    }

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
