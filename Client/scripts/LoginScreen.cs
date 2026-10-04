using System;
using Godot;
using Noobietoria.Platform;

namespace Noobietoria.Client;

/// <summary>
/// Roblox-style entry point: the app opens here, the player logs into
/// their platform account and lands in the game list. No server addresses
/// are exposed — the platform assigns a place when a game is played.
///
/// Development escape hatch: launching with `--dev-connect host:port`
/// skips the portal and connects straight to a server, like before.
/// </summary>
public partial class LoginScreen : Control
{
    private LineEdit _usernameInput = null!;
    private LineEdit _passwordInput = null!;
    private Button _loginButton = null!;
    private Button _registerButton = null!;
    private Label _statusLabel = null!;

    private PlatformClient Platform => new(ClientState.PlatformUrl);

    public override void _Ready()
    {
        _usernameInput = GetNode<LineEdit>("%UsernameInput");
        _passwordInput = GetNode<LineEdit>("%PasswordInput");
        _loginButton = GetNode<Button>("%LoginButton");
        _registerButton = GetNode<Button>("%RegisterButton");
        _statusLabel = GetNode<Label>("%StatusLabel");

        _usernameInput.Text = ClientState.Account?.Username ?? ClientState.UserName;

        _loginButton.Pressed += OnLoginPressed;
        _registerButton.Pressed += OnRegisterPressed;

        // Dev escape hatch: --dev-connect host:port
        foreach (string arg in OS.GetCmdlineUserArgs())
        {
            if (arg.StartsWith("--dev-connect=", StringComparison.Ordinal))
            {
                var target = arg["--dev-connect=".Length..].Split(':');
                if (target.Length == 2 && int.TryParse(target[1], out int port))
                {
                    ClientState.UserName = _usernameInput.Text.Trim();
                    ClientState.Address = target[0];
                    ClientState.Port = port;
                    ClientState.Ticket = string.Empty;
                    CallDeferred(nameof(EnterGame));
                    return;
                }
            }
        }

        if (!string.IsNullOrEmpty(ClientState.LastError))
        {
            _statusLabel.Text = ClientState.LastError;
            ClientState.LastError = null;
        }
    }

    public override void _ExitTree()
    {
        _loginButton.Pressed -= OnLoginPressed;
        _registerButton.Pressed -= OnRegisterPressed;
    }

    private async void OnLoginPressed()
    {
        string username = _usernameInput.Text.Trim();
        if (username.Length == 0)
        {
            _statusLabel.Text = "Enter your username.";
            return;
        }

        SetBusy("Logging in…");
        try
        {
            ClientState.Account = await Platform.LoginAsync(username, _passwordInput.Text).ConfigureAwait(true);
            ClientState.UserName = ClientState.Account.Username;
            GetTree().ChangeSceneToFile("res://scenes/Home.tscn");
        }
        catch (Exception e)
        {
            SetBusy($"Login failed: {e.Message}");
        }
    }

    private async void OnRegisterPressed()
    {
        string username = _usernameInput.Text.Trim();
        if (username.Length == 0)
        {
            _statusLabel.Text = "Enter a username.";
            return;
        }

        SetBusy("Creating account…");
        try
        {
            ClientState.Account = await Platform.RegisterAsync(username, _passwordInput.Text).ConfigureAwait(true);
            ClientState.UserName = ClientState.Account.Username;
            GetTree().ChangeSceneToFile("res://scenes/Home.tscn");
        }
        catch (Exception e)
        {
            SetBusy($"Could not create the account: {e.Message}");
        }
    }

    private void EnterGame() => GetTree().ChangeSceneToFile("res://scenes/Main.tscn");

    private void SetBusy(string message)
    {
        _statusLabel.Text = message;
        _loginButton.Disabled = true;
        _registerButton.Disabled = true;
    }
}
