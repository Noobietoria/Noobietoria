using Godot;

namespace Noobietoria.DedicatedServer;

/// <summary>
/// Sits at /root/Main/Players, mirroring the client's node layout so that
/// avatar state RPCs have a matching node path on the server. The server does
/// not simulate avatars yet (client-authoritative v0); server-side movement
/// validation can hook in here later.
/// </summary>
public partial class PlayerRelay : Node3D
{
    [Rpc(MultiplayerApi.RpcMode.AnyPeer, CallLocal = false,
        TransferMode = MultiplayerPeer.TransferModeEnum.Unreliable)]
    private void SyncPlayerState(Vector3 position, float yaw)
    {
        // Accepted and ignored for now.
    }
}
