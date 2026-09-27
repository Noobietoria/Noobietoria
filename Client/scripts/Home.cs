using System;
using Godot;
using Noobietoria.Platform;

namespace Noobietoria.Client;

/// <summary>
/// The portal home: the game catalog of the platform. Pressing a game
/// joins it immediately — the platform assigns a place and hands back a
/// signed join ticket, and the client connects without any server details
/// being exposed.
/// </summary>
public partial class Home : Control
{
    private PlatformClient Platform => new(ClientState.PlatformUrl);

    private Label _userLabel = null!;
    private VBoxContainer _gamesBox = null!;
    private Label _statusLabel = null!;

    public override void _Ready()
    {
        _userLabel = GetNode<Label>("%UserLabel");
        _gamesBox = GetNode<VBoxContainer>("%GamesBox");
        _statusLabel = GetNode<Label>("%StatusLabel");

        // No platform session (launched straight into this scene) — go log in.
        if (ClientState.Account == null)
        {
            GetTree().ChangeSceneToFile("res://scenes/LoginScreen.tscn");
            return;
        }

        GetNode<Button>("%LogoutButton").Pressed += OnLogoutPressed;
        _userLabel.Text = ClientState.Account.Username;

        _ = LoadGamesAsync();
    }

    public override void _ExitTree()
    {
        var logout = GetNodeOrNull<Button>("%LogoutButton");
        if (logout != null)
            logout.Pressed -= OnLogoutPressed;
    }

    private async System.Threading.Tasks.Task LoadGamesAsync()
    {
        _statusLabel.Text = "Loading games…";
        try
        {
            var games = await Platform.GetGamesAsync(ClientState.Account!.Token).ConfigureAwait(true);

            foreach (var child in _gamesBox.GetChildren())
                child.QueueFree();

            if (games.Count == 0)
            {
                _statusLabel.Text = "No games are live yet — check back soon!";
                return;
            }

            foreach (var game in games)
            {
                var card = new Button
                {
                    Text = $"{game.Title}\n{game.Description}   ·   {game.PlayersOnline} online",
                    CustomMinimumSize = new Vector2(0, 76),
                };
                var captured = game;
                card.Pressed += () => JoinGame(captured);
                _gamesBox.AddChild(card);
            }

            _statusLabel.Text = string.Empty;
        }
        catch (Exception e)
        {
            _statusLabel.Text = $"Could not load games: {e.Message}";
        }
    }

    private async void JoinGame(GameListing game)
    {
        _statusLabel.Text = $"Joining {game.Title}…";
        SetCardsEnabled(false);

        try
        {
            var join = await Platform.RequestJoinAsync(game.GameId, ClientState.Account!.Token)
                .ConfigureAwait(true);

            ClientState.GameTitle = game.Title;
            ClientState.WorldId = join.WorldId;
            ClientState.Ticket = join.Ticket;
            ClientState.Address = join.Host;
            ClientState.Port = join.Port;
            ClientState.UserName = ClientState.Account.Username;

            GetTree().ChangeSceneToFile("res://scenes/Main.tscn");
        }
        catch (Exception e)
        {
            _statusLabel.Text = $"Could not join {game.Title}: {e.Message}";
            SetCardsEnabled(true);
        }
    }

    private void OnLogoutPressed()
    {
        ClientState.ClearSession();
        GetTree().ChangeSceneToFile("res://scenes/LoginScreen.tscn");
    }

    private void SetCardsEnabled(bool enabled)
    {
        foreach (var child in _gamesBox.GetChildren())
        {
            if (child is Button card)
                card.Disabled = !enabled;
        }
    }
}
