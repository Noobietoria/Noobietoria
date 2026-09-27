using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using Godot;

namespace Noobietoria.Studio;

/// <summary>
/// Minimal UGC map editor: a snap-to-grid block builder with JSON save/load
/// under res://maps (the Studio/maps folder in the repo). Prefabs, scripting
/// and publishing maps to servers come later.
/// </summary>
public partial class StudioMain : Node3D
{
    private const int Bounds = 32;      // horizontal build limit in blocks
    private const int MaxHeight = 24;
    private const float RayLength = 60f;
    private const float LookSpeed = 0.004f;

    private static readonly string[] BlockTypes = { "grass", "stone", "wood", "brick" };
    private static readonly Color[] BlockColors =
    {
        new(0.36f, 0.62f, 0.28f),
        new(0.56f, 0.57f, 0.60f),
        new(0.63f, 0.47f, 0.27f),
        new(0.63f, 0.27f, 0.22f),
    };

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        PropertyNameCaseInsensitive = true,
        WriteIndented = true,
    };

    private sealed record MapFile(int Version, string Name, List<MapBlock> Blocks);
    private sealed record MapBlock(int X, int Y, int Z, string Type);

    private readonly Dictionary<Vector3I, string> _blocks = new();
    private readonly Dictionary<Vector3I, StaticBody3D> _blockNodes = new();
    private readonly Dictionary<string, StandardMaterial3D> _materials = new();

    private BoxMesh _blockMesh = null!;
    private BoxShape3D _blockShape = null!;
    private MeshInstance3D _preview = null!;
    private StaticBody3D _ground = null!;
    private Camera3D _camera = null!;

    private OptionButton _blockPicker = null!;
    private LineEdit _mapNameInput = null!;
    private Label _statusLabel = null!;

    private float _yaw = Mathf.Pi / 4f;
    private float _pitch = -0.55f;
    private bool _orbiting;
    private Vector2 _orbitPressPosition;

    public override void _Ready()
    {
        _camera = GetNode<Camera3D>("%Cam");
        _ground = GetNode<StaticBody3D>("%Ground");
        _blockPicker = GetNode<OptionButton>("%BlockPicker");
        _mapNameInput = GetNode<LineEdit>("%MapNameInput");
        _statusLabel = GetNode<Label>("%StatusLabel");

        GetNode<DirectionalLight3D>("%Sun").RotationDegrees = new Vector3(-55f, 30f, 0f);
        _camera.Position = new Vector3(14f, 12f, 14f);

        _blockMesh = new BoxMesh { Size = Vector3.One };
        _blockShape = new BoxShape3D { Size = Vector3.One };
        for (int i = 0; i < BlockTypes.Length; i++)
        {
            _blockPicker.AddItem(BlockTypes[i], i);
            _materials[BlockTypes[i]] = new StandardMaterial3D { AlbedoColor = BlockColors[i] };
        }
        _blockPicker.Select(0);

        _preview = new MeshInstance3D
        {
            Mesh = _blockMesh,
            MaterialOverride = new StandardMaterial3D
            {
                AlbedoColor = new Color(1f, 1f, 1f, 0.3f),
                Transparency = BaseMaterial3D.TransparencyEnum.Alpha,
                ShadingMode = BaseMaterial3D.ShadingModeEnum.Unshaded,
            },
            Visible = false,
        };
        AddChild(_preview);

        GetNode<Button>("%NewButton").Pressed += OnNewPressed;
        GetNode<Button>("%SaveButton").Pressed += OnSavePressed;
        GetNode<Button>("%LoadButton").Pressed += OnLoadPressed;

        DirAccess.MakeDirRecursiveAbsolute(ProjectSettings.GlobalizePath("res://maps"));
        UpdateStatus("LMB place · RMB drag rotate · RMB click remove · WASD/QE move · Shift boost");
    }

    public override void _Process(double delta)
    {
        float dt = (float)delta;

        _camera.Basis = Basis.FromEuler(new Vector3(_pitch, _yaw, 0.0f));

        if (!_mapNameInput.HasFocus())
        {
            bool fast = Input.IsKeyPressed(Key.Shift);
            Vector3 move = Vector3.Zero;
            if (Input.IsKeyPressed(Key.W)) move -= _camera.Basis.Z;
            if (Input.IsKeyPressed(Key.S)) move += _camera.Basis.Z;
            if (Input.IsKeyPressed(Key.A)) move -= _camera.Basis.X;
            if (Input.IsKeyPressed(Key.D)) move += _camera.Basis.X;
            if (Input.IsKeyPressed(Key.E)) move += Vector3.Up;
            if (Input.IsKeyPressed(Key.Q)) move -= Vector3.Up;

            if (move.LengthSquared() > 0.0f)
                _camera.Position += move.Normalized() * (fast ? 24.0f : 8.0f) * dt;

            _camera.Position = new Vector3(
                Mathf.Clamp(_camera.Position.X, -Bounds - 8f, Bounds + 8f),
                Mathf.Max(0.6f, _camera.Position.Y),
                Mathf.Clamp(_camera.Position.Z, -Bounds - 8f, Bounds + 8f));
        }

        UpdatePreview();
    }

    public override void _UnhandledInput(InputEvent @event)
    {
        switch (@event)
        {
            case InputEventMouseButton mb:
                if (mb.ButtonIndex == MouseButton.Right)
                {
                    if (mb.Pressed)
                    {
                        _orbiting = true;
                        _orbitPressPosition = mb.Position;
                    }
                    else
                    {
                        _orbiting = false;
                        // A clean right-click (no drag) removes the hovered block.
                        if (mb.Position.DistanceTo(_orbitPressPosition) < 6.0f)
                            TryRemoveBlock();
                    }
                }
                else if (mb.ButtonIndex == MouseButton.Left && mb.Pressed)
                {
                    TryPlaceBlock();
                }
                break;

            case InputEventMouseMotion motion when _orbiting:
                _yaw -= motion.Relative.X * LookSpeed;
                _pitch = Mathf.Clamp(_pitch - motion.Relative.Y * LookSpeed, -1.5f, 1.5f);
                break;
        }
    }

    // ---- world editing ----

    private bool CastRay(out Vector3 point, out Vector3 normal, out bool hitGround)
    {
        point = default;
        normal = Vector3.Up;
        hitGround = false;

        Vector2 mouse = GetViewport().GetMousePosition();
        Vector3 from = _camera.ProjectRayOrigin(mouse);
        Vector3 to = from + _camera.ProjectRayNormal(mouse) * RayLength;

        Godot.Collections.Dictionary hit = GetWorld3D().DirectSpaceState
            .IntersectRay(PhysicsRayQueryParameters3D.Create(from, to));
        if (hit.Count == 0)
            return false;

        point = hit["position"].AsVector3();
        normal = hit["normal"].AsVector3();
        hitGround = hit["collider"].As<GodotObject>() == _ground;
        return true;
    }

    private static Vector3I PlaceCell(Vector3 point, Vector3 normal)
        => (Vector3I)(point + normal * 0.01f).Floor();

    private static Vector3I HitCell(Vector3 point, Vector3 normal)
        => (Vector3I)(point - normal * 0.01f).Floor();

    private static bool InBounds(Vector3I cell)
        => Mathf.Abs(cell.X) <= Bounds && Mathf.Abs(cell.Z) <= Bounds
            && cell.Y >= 0 && cell.Y <= MaxHeight;

    private void TryPlaceBlock()
    {
        if (!CastRay(out Vector3 point, out Vector3 normal, out _))
            return;

        Vector3I cell = PlaceCell(point, normal);
        if (!InBounds(cell))
        {
            UpdateStatus("Out of build bounds.");
            return;
        }
        if (_blocks.ContainsKey(cell))
            return;

        string type = BlockTypes[Mathf.Max(0, _blockPicker.Selected)];
        AddBlock(cell, type);
        UpdateStatus($"{_blocks.Count} blocks · placed {type} at {cell}");
    }

    private void TryRemoveBlock()
    {
        if (!CastRay(out Vector3 point, out Vector3 normal, out bool hitGround))
            return;
        if (hitGround)
            return;

        Vector3I cell = HitCell(point, normal);
        if (!_blocks.ContainsKey(cell))
            return;

        RemoveBlock(cell);
        UpdateStatus($"{_blocks.Count} blocks · removed block at {cell}");
    }

    private void UpdatePreview()
    {
        if (!CastRay(out Vector3 point, out Vector3 normal, out _))
        {
            _preview.Visible = false;
            return;
        }

        Vector3I cell = PlaceCell(point, normal);
        if (!InBounds(cell) || _blocks.ContainsKey(cell))
        {
            _preview.Visible = false;
            return;
        }

        _preview.Position = (Vector3)cell + new Vector3(0.5f, 0.5f, 0.5f);
        _preview.Visible = true;
    }

    private void AddBlock(Vector3I cell, string type)
    {
        var body = new StaticBody3D { Position = (Vector3)cell + new Vector3(0.5f, 0.5f, 0.5f) };
        body.AddChild(new MeshInstance3D { Mesh = _blockMesh, MaterialOverride = _materials[type] });
        body.AddChild(new CollisionShape3D { Shape = _blockShape });
        GetNode<Node3D>("%World").AddChild(body);

        _blocks[cell] = type;
        _blockNodes[cell] = body;
    }

    private void RemoveBlock(Vector3I cell)
    {
        _blocks.Remove(cell);
        if (_blockNodes.Remove(cell, out StaticBody3D? node))
            node.QueueFree();
    }

    // ---- toolbar ----

    private void OnNewPressed()
    {
        foreach (StaticBody3D node in _blockNodes.Values)
            node.QueueFree();
        _blockNodes.Clear();
        _blocks.Clear();
        UpdateStatus("Cleared. 0 blocks.");
    }

    private void OnSavePressed()
    {
        string name = SanitizeMapName(_mapNameInput.Text);
        var file = new MapFile(1, name,
            _blocks.Select(kv => new MapBlock(kv.Key.X, kv.Key.Y, kv.Key.Z, kv.Value)).ToList());

        try
        {
            string path = ProjectSettings.GlobalizePath($"res://maps/{name}.json");
            File.WriteAllText(path, JsonSerializer.Serialize(file, JsonOptions));
            UpdateStatus($"Saved {_blocks.Count} blocks to maps/{name}.json");
        }
        catch (Exception e)
        {
            UpdateStatus($"Save failed: {e.Message}");
        }
    }

    private void OnLoadPressed()
    {
        string name = SanitizeMapName(_mapNameInput.Text);
        string path = ProjectSettings.GlobalizePath($"res://maps/{name}.json");
        if (!File.Exists(path))
        {
            UpdateStatus($"No map at maps/{name}.json");
            return;
        }

        try
        {
            MapFile? file = JsonSerializer.Deserialize<MapFile>(File.ReadAllText(path), JsonOptions);
            if (file is null || file.Version != 1)
            {
                UpdateStatus($"Unsupported map format in {name}.json");
                return;
            }

            OnNewPressed();
            foreach (MapBlock block in file.Blocks)
            {
                var cell = new Vector3I(block.X, block.Y, block.Z);
                if (InBounds(cell) && !_blocks.ContainsKey(cell) && _materials.ContainsKey(block.Type))
                    AddBlock(cell, block.Type);
            }
            UpdateStatus($"Loaded {file.Name}: {_blocks.Count} blocks.");
        }
        catch (Exception e)
        {
            UpdateStatus($"Load failed: {e.Message}");
        }
    }

    private static string SanitizeMapName(string raw)
    {
        var sb = new System.Text.StringBuilder();
        foreach (char c in raw.Trim().ToLowerInvariant())
            sb.Append(char.IsAsciiLetterOrDigit(c) ? c : '-');
        string name = sb.ToString().Trim('-');
        return name.Length > 0 ? name : "untitled";
    }

    private void UpdateStatus(string message)
    {
        _statusLabel.Text = message;
        GD.Print($"[studio] {message}");
    }
}
