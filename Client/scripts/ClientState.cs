namespace Noobietoria.Client;

/// <summary>Carries connection settings between the menu scene and the game scene.</summary>
public static class ClientState
{
    public static string UserName { get; set; } = "Newbie";
    public static string Address { get; set; } = "127.0.0.1";
    public static int Port { get; set; } = Noobietoria.Shared.Net.DefaultPort;

    /// <summary>Message the menu shows once, e.g. why the last session ended.</summary>
    public static string? LastError { get; set; }

    /// <summary>Platform join ticket for the current session (empty in dev direct-connect).</summary>
    public static string Ticket { get; set; } = "";

    /// <summary>Title of the platform game being joined (portal flow).</summary>
    public static string GameTitle { get; set; } = "";
}
