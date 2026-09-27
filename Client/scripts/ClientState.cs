using Noobietoria.Platform;

using Noobietoria.Shared;

namespace Noobietoria.Client;

/// <summary>
/// Session state shared across the client scenes: the platform account and
/// the place assignment the player is currently in.
///
/// The platform backend URL is taken from the NOOBIETORIA_PLATFORM_URL
/// environment variable when set; empty means the offline dev platform
/// (any credentials log in, sample games are served).
/// </summary>
public static class ClientState
{
    /// <summary>Production platform API base URL for release builds.</summary>
    public const string ProductionPlatformUrl = "";

    public static string UserName { get; set; } = "Newbie";
    public static string Address { get; set; } = "127.0.0.1";
    public static int Port { get; set; } = Net.DefaultPort;

    /// <summary>Message the previous scene shows once (e.g. why the last session ended).</summary>
    public static string? LastError { get; set; }

    /// <summary>Platform join ticket for the current session (empty in dev direct-connect).</summary>
    public static string Ticket { get; set; } = "";

    /// <summary>Title of the platform game being joined (portal flow).</summary>
    public static string GameTitle { get; set; } = "";

    /// <summary>World the platform assigned for this session.</summary>
    public static string WorldId { get; set; } = "";

    /// <summary>The logged-in platform account, or null before login.</summary>
    public static PlatformAccount? Account { get; set; }

    /// <summary>Platform API base URL — env override, empty means offline dev platform.</summary>
    public static string PlatformUrl { get; } =
        Godot.OS.GetEnvironment("NOOBIETORIA_PLATFORM_URL") is { Length: > 0 } url ? url : ProductionPlatformUrl;

    /// <summary>True while a platform session is active (portal flow).</summary>
    public static bool HasPlatformSession => Account != null;

    public static void ClearSession()
    {
        Account = null;
        Ticket = string.Empty;
        GameTitle = string.Empty;
        WorldId = string.Empty;
        UserName = "Newbie";
    }
}
