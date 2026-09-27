using Godot;

namespace Noobietoria.Client;

/// <summary>
/// One avatar in the world. Every peer spawns an instance for every connected
/// player; only the owning peer (the multiplayer authority of the node) runs
/// physics and broadcasts its state.
/// </summary>
public partial class Player : CharacterBody3D
{
    private const float Gravity = 14.0f;
    private const float Speed = 6.0f;
    private const float JumpVelocity = 5.0f;
    private const float SyncInterval = 0.05f;
    private const float FallResetHeight = -20.0f;

    /// <summary>Peer id of the player who owns this avatar (set before AddChild).</summary>
    public int PeerId { get; set; }

    /// <summary>Name shown on the name tag (set before AddChild).</summary>
    public string DisplayName { get; set; } = "Player";

    private float _syncTimer;
    private Vector3 _remoteTargetPosition;
    private float _remoteTargetYaw;
    private bool _hasRemoteState;

    public static Vector3 SpawnPointFor(int peerId)
    {
        float ring = peerId % 8;
        float circle = (peerId / 8) % 4;
        return new Vector3(-10.5f + ring * 3.0f, 1.0f, -4.5f + circle * 3.0f);
    }

    public override void _Ready()
    {
        SetMultiplayerAuthority(PeerId);
        GetNode<Label3D>("%NameTag").Text = DisplayName;
        Position = SpawnPointFor(PeerId);
    }

    public override void _PhysicsProcess(double delta)
    {
        float dt = (float)delta;

        if (IsMultiplayerAuthority())
        {
            SimulateLocal(dt);

            _syncTimer -= dt;
            if (_syncTimer <= 0.0f)
            {
                _syncTimer = SyncInterval;
                Rpc(nameof(SyncPlayerState), Position, Rotation.Y);
            }
        }
        else if (_hasRemoteState)
        {
            float weight = Mathf.Min(1.0f, dt * 12.0f);
            Position = Position.Lerp(_remoteTargetPosition, weight);
            Rotation = new Vector3(0.0f, Mathf.LerpAngle(Rotation.Y, _remoteTargetYaw, weight), 0.0f);
        }
    }

    private void SimulateLocal(float dt)
    {
        bool chatFocused = Game.ChatFocused;
        Vector3 planar = Vector3.Zero;

        if (!chatFocused)
        {
            if (Input.IsKeyPressed(Key.A) || Input.IsKeyPressed(Key.Left)) planar.X -= 1.0f;
            if (Input.IsKeyPressed(Key.D) || Input.IsKeyPressed(Key.Right)) planar.X += 1.0f;
            if (Input.IsKeyPressed(Key.W) || Input.IsKeyPressed(Key.Up)) planar.Z -= 1.0f;
            if (Input.IsKeyPressed(Key.S) || Input.IsKeyPressed(Key.Down)) planar.Z += 1.0f;
            planar = planar.Normalized() * Speed;
        }

        Velocity = new Vector3(planar.X, Velocity.Y - Gravity * dt, planar.Z);

        if (!chatFocused && Input.IsKeyPressed(Key.Space) && IsOnFloor())
            Velocity = new Vector3(Velocity.X, JumpVelocity, Velocity.Z);

        MoveAndSlide();

        if (Position.Y < FallResetHeight)
        {
            Position = SpawnPointFor(PeerId);
            Velocity = Vector3.Zero;
        }
    }

    [Rpc(CallLocal = false, TransferMode = MultiplayerPeer.TransferModeEnum.Unreliable)]
    private void SyncPlayerState(Vector3 position, float yaw)
    {
        _remoteTargetPosition = position;
        _remoteTargetYaw = yaw;
        _hasRemoteState = true;
    }
}
