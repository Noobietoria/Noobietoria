namespace Noobietoria.Shared;

/// <summary>Networking constants shared by the Client and the DedicatedServer.</summary>
public static class Net
{
    /// <summary>Default ENet port the dedicated server listens on.</summary>
    public const int DefaultPort = 24565;

    /// <summary>Maximum number of simultaneous players on one server.</summary>
    public const int MaxPlayers = 32;

    /// <summary>Clients and servers must agree on this tag or the handshake is rejected.</summary>
    public const string ProtocolTag = "noobietoria-1";
}

/// <summary>
/// RPC method names. Declared as strings (instead of relying on the generated
/// <c>MethodName</c> classes) so the Client and the DedicatedServer — separate
/// Godot assemblies — agree on one source of truth.
/// </summary>
public static class RpcMethod
{
    public const string SubmitHandshake = "SubmitHandshake";
    public const string HandshakeAccepted = "HandshakeAccepted";
    public const string HandshakeRejected = "HandshakeRejected";
    public const string PlayerJoined = "PlayerJoined";
    public const string PlayerLeft = "PlayerLeft";
    public const string ReceiveChat = "ReceiveChat";
    public const string SendChat = "SendChat";
    public const string SyncPlayerState = "SyncPlayerState";
}
