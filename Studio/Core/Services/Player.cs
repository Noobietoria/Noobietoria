using System;
using System.Collections.Generic;
using System.Linq;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// PlayerService tracks who joins, who leaves, where they go, player
    /// names, their country and their avatar/thumbnail/character assets.
    /// The host (server) feeds join/leave through TrackJoin/TrackLeave; the
    /// remaining members are the Luau-facing API.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#playerservice
    /// </summary>
    public class PlayerService
    {
        private sealed class PlayerRecord
        {
            public int UserId;
            public string Username = "";
            public string Country = "";
            public string Avatar = "";
            public string AvatarUrl = "";
            public string Thumbnail = "";
            public string ThumbnailUrl = "";
            public string Character = "";
            public string CharacterUrl = "";
            public Vector3Data Coors;
        }

        /// <summary>Read-only player snapshot returned by GetPlayer.</summary>
        public sealed record PlayerSnapshot(
            int UserId, string Username, string Country,
            string Avatar, string AvatarUrl, string Thumbnail, string ThumbnailUrl,
            string Character, string CharacterUrl, Vector3Data Coors);

        private readonly object _lock = new();
        private readonly Dictionary<string, PlayerRecord> _byName = new(StringComparer.Ordinal);
        private readonly Dictionary<int, PlayerRecord> _byId = new();
        private int _nextUserId = 1;

        /// <summary>Raised when a player joins the game (host wiring).</summary>
        public event Action<string>? Joined;

        /// <summary>Raised when a player leaves the game (host wiring).</summary>
        public event Action<string>? Left;

        /// <summary>Registers a joining player and assigns their UserId.</summary>
        public int TrackJoin(
            string username,
            string country = "",
            string avatar = "",
            string avatarUrl = "",
            string thumbnail = "",
            string thumbnailUrl = "",
            string character = "",
            string characterUrl = "")
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));

            lock (_lock)
            {
                if (_byName.ContainsKey(username))
                    throw new InvalidOperationException($"Player '{username}' is already in the game.");

                var record = new PlayerRecord
                {
                    UserId = _nextUserId++,
                    Username = username,
                    Country = country,
                    Avatar = avatar,
                    AvatarUrl = avatarUrl,
                    Thumbnail = thumbnail,
                    ThumbnailUrl = thumbnailUrl,
                    Character = character,
                    CharacterUrl = characterUrl,
                };
                _byName[username] = record;
                _byId[record.UserId] = record;
                Joined?.Invoke(username);
                return record.UserId;
            }
        }

        /// <summary>Registers a leaving player.</summary>
        public bool TrackLeave(string username)
        {
            lock (_lock)
            {
                if (!_byName.Remove(username, out var record))
                    return false;
                _byId.Remove(record.UserId);
                Left?.Invoke(username);
                return true;
            }
        }

        /// <summary>PlayerService.GetPlayers() — usernames in join order.</summary>
        public IReadOnlyList<string> GetPlayers()
        {
            lock (_lock)
            {
                return _byName.Values.OrderBy(r => r.UserId).Select(r => r.Username).ToArray();
            }
        }

        /// <summary>PlayerService.GetPlayer("Username") — full snapshot or null.</summary>
        public PlayerSnapshot? GetPlayer(string username)
        {
            lock (_lock)
            {
                return _byName.TryGetValue(username, out var r)
                    ? new PlayerSnapshot(r.UserId, r.Username, r.Country, r.Avatar, r.AvatarUrl,
                        r.Thumbnail, r.ThumbnailUrl, r.Character, r.CharacterUrl, r.Coors)
                    : null;
            }
        }

        /// <summary>PlayerService.GetPlayerName(UserId)</summary>
        public string? GetPlayerName(int userId)
        {
            lock (_lock)
            {
                return _byId.TryGetValue(userId, out var r) ? r.Username : null;
            }
        }

        /// <summary>PlayerService.GetPlayerCountry(UserId)</summary>
        public string? GetPlayerCountry(int userId)
        {
            lock (_lock)
            {
                return _byId.TryGetValue(userId, out var r) ? r.Country : null;
            }
        }

        /// <summary>PlayerService.GetPlayerAvatar(UserId)</summary>
        public string? GetPlayerAvatar(int userId) => WithUser(userId, r => r.Avatar);

        /// <summary>PlayerService.GetPlayerAvatarUrl(UserId)</summary>
        public string? GetPlayerAvatarUrl(int userId) => WithUser(userId, r => r.AvatarUrl);

        /// <summary>PlayerService.GetPlayerThumbnail(UserId)</summary>
        public string? GetPlayerThumbnail(int userId) => WithUser(userId, r => r.Thumbnail);

        /// <summary>PlayerService.GetPlayerThumbnailUrl(UserId)</summary>
        public string? GetPlayerThumbnailUrl(int userId) => WithUser(userId, r => r.ThumbnailUrl);

        /// <summary>PlayerService.GetPlayerCharacter(UserId)</summary>
        public string? GetPlayerCharacter(int userId) => WithUser(userId, r => r.Character);

        /// <summary>PlayerService.GetPlayerCharacterUrl(UserId)</summary>
        public string? GetPlayerCharacterUrl(int userId) => WithUser(userId, r => r.CharacterUrl);

        /// <summary>PlayerService.GetCoors("Username")</summary>
        public Vector3Data GetCoors(string username)
        {
            lock (_lock)
            {
                return RequirePlayer(username).Coors;
            }
        }

        /// <summary>PlayerService.Teleport("Username", Coors)</summary>
        public void Teleport(string username, Vector3Data coors)
        {
            lock (_lock)
            {
                RequirePlayer(username).Coors = coors;
            }
        }

        private PlayerRecord RequirePlayer(string username)
        {
            if (!_byName.TryGetValue(username, out var record))
                throw new InvalidOperationException($"No player named '{username}' is in the game.");
            return record;
        }

        private string? WithUser(int userId, Func<PlayerRecord, string> selector)
        {
            lock (_lock)
            {
                return _byId.TryGetValue(userId, out var r) ? selector(r) : null;
            }
        }
    }

    /// <summary>
    /// SpawnService manages the named spawn points of Players and NPCs.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#spawnservice
    /// </summary>
    public class SpawnService
    {
        /// <summary>Spawn point snapshot (GetSpawn / GetAllSpawns).</summary>
        public sealed record SpawnSnapshot(string Name, Vector3Data Position);

        private readonly object _lock = new();
        private readonly Dictionary<string, Vector3Data> _spawns = new(StringComparer.Ordinal);
        private readonly Dictionary<string, string> _assigned = new(StringComparer.Ordinal);

        /// <summary>Raised on Respawn/RespawnAt: (targetType, username, spawnName, position).</summary>
        public event Action<string, string, string, Vector3Data>? Respawned;

        /// <summary>SpawnService.CreateSpawn(SpawnName, Position)</summary>
        public void CreateSpawn(string spawnName, Vector3Data position)
        {
            RequireSpawnName(spawnName);
            lock (_lock)
            {
                if (!_spawns.TryAdd(spawnName, position))
                    throw new InvalidOperationException($"Spawn '{spawnName}' already exists.");
            }
        }

        /// <summary>SpawnService.DeleteSpawn(SpawnName)</summary>
        public void DeleteSpawn(string spawnName)
        {
            RequireSpawnName(spawnName);
            lock (_lock)
            {
                if (!_spawns.Remove(spawnName))
                    throw new InvalidOperationException($"No spawn named '{spawnName}'.");
            }
        }

        /// <summary>SpawnService.SetSpawn(SpawnName, Position)</summary>
        public void SetSpawn(string spawnName, Vector3Data position)
        {
            RequireSpawnName(spawnName);
            lock (_lock)
            {
                if (!_spawns.ContainsKey(spawnName))
                    throw new InvalidOperationException($"No spawn named '{spawnName}'.");
                _spawns[spawnName] = position;
            }
        }

        /// <summary>SpawnService.GetSpawn(SpawnName)</summary>
        public Vector3Data? GetSpawn(string spawnName)
        {
            RequireSpawnName(spawnName);
            lock (_lock)
            {
                return _spawns.TryGetValue(spawnName, out var position) ? position : null;
            }
        }

        /// <summary>SpawnService.GetAllSpawns()</summary>
        public IReadOnlyList<SpawnSnapshot> GetAllSpawns()
        {
            lock (_lock)
            {
                return _spawns.Select(kv => new SpawnSnapshot(kv.Key, kv.Value)).ToArray();
            }
        }

        /// <summary>SpawnService.AssignSpawn(TargetType, Username, SpawnName)</summary>
        public void AssignSpawn(string targetType, string username, string spawnName)
        {
            RequireTargetType(targetType);
            RequireSpawnName(spawnName);
            lock (_lock)
            {
                if (!_spawns.ContainsKey(spawnName))
                    throw new InvalidOperationException($"No spawn named '{spawnName}'.");
                _assigned[Key(targetType, username)] = spawnName;
            }
        }

        /// <summary>
        /// SpawnService.RespawnAt(TargetType, Username, SpawnName) — assigns
        /// the spawn and raises <see cref="Respawned"/> immediately.
        /// </summary>
        public void RespawnAt(string targetType, string username, string spawnName)
        {
            AssignSpawn(targetType, username, spawnName);
            lock (_lock)
            {
                Respawned?.Invoke(NormalizeTarget(targetType), username, spawnName, _spawns[spawnName]);
            }
        }

        /// <summary>
        /// SpawnService.Respawn(TargetType, Username) — respawns at the
        /// currently assigned spawn.
        /// </summary>
        public void Respawn(string targetType, string username)
        {
            RequireTargetType(targetType);
            lock (_lock)
            {
                string key = Key(targetType, username);
                if (!_assigned.TryGetValue(key, out var spawnName) || !_spawns.TryGetValue(spawnName, out var position))
                    throw new InvalidOperationException(
                        $"No spawn assigned to {NormalizeTarget(targetType)} '{username}' — call AssignSpawn first.");

                Respawned?.Invoke(NormalizeTarget(targetType), username, spawnName, position);
            }
        }

        internal string? AssignedSpawnOf(string targetType, string username)
        {
            lock (_lock)
            {
                return _assigned.TryGetValue(Key(targetType, username), out var spawnName) ? spawnName : null;
            }
        }

        private static string Key(string targetType, string username) =>
            NormalizeTarget(targetType) + "\u0001" + username;

        private static string NormalizeTarget(string targetType)
        {
            if (string.Equals(targetType, "Player", StringComparison.OrdinalIgnoreCase)) return "Player";
            if (string.Equals(targetType, "NPC", StringComparison.OrdinalIgnoreCase)) return "NPC";
            throw new ArgumentException(
                $"TargetType '{targetType}' is not supported (use \"Player\" or \"NPC\").", nameof(targetType));
        }

        private static void RequireSpawnName(string spawnName)
        {
            if (string.IsNullOrWhiteSpace(spawnName))
                throw new ArgumentException("SpawnName must not be empty.", nameof(spawnName));
        }

        private static void RequireTargetType(string targetType)
        {
            NormalizeTarget(targetType);
        }
    }

    /// <summary>
    /// TeleportService moves players between registered zones or servers.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#teleportservice
    /// </summary>
    public class TeleportService
    {
        /// <summary>Zone snapshot (GetZone / GetZones).</summary>
        public sealed record ZoneSnapshot(string ZoneName, string? ServerId, Vector3Data? SpawnPosition);

        /// <summary>Teleport record delivered to OnTeleport callbacks.</summary>
        public sealed record TeleportEvent(string Username, string Destination, string? ServerId, Vector3Data? Position);

        private readonly object _lock = new();
        private readonly Dictionary<string, (string? ServerId, Vector3Data? SpawnPosition)> _zones =
            new(StringComparer.OrdinalIgnoreCase);
        private readonly Dictionary<string, List<Action<TeleportEvent>>> _teleportCallbacks =
            new(StringComparer.Ordinal);

        /// <summary>TeleportService.RegisterZone(ZoneName, ServerId, SpawnPosition)</summary>
        public void RegisterZone(string zoneName, string? serverId = null, Vector3Data? spawnPosition = null)
        {
            RequireZoneName(zoneName);
            lock (_lock)
            {
                if (!_zones.TryAdd(zoneName, (serverId, spawnPosition)))
                    throw new InvalidOperationException($"Zone '{zoneName}' is already registered.");
            }
        }

        /// <summary>TeleportService.UnregisterZone(ZoneName)</summary>
        public void UnregisterZone(string zoneName)
        {
            RequireZoneName(zoneName);
            lock (_lock)
            {
                if (!_zones.Remove(zoneName))
                    throw new InvalidOperationException($"No zone named '{zoneName}'.");
            }
        }

        /// <summary>TeleportService.GetZone(ZoneName)</summary>
        public ZoneSnapshot? GetZone(string zoneName)
        {
            RequireZoneName(zoneName);
            lock (_lock)
            {
                return _zones.TryGetValue(zoneName, out var zone)
                    ? new ZoneSnapshot(zoneName, zone.ServerId, zone.SpawnPosition)
                    : null;
            }
        }

        /// <summary>TeleportService.GetZones()</summary>
        public IReadOnlyList<ZoneSnapshot> GetZones()
        {
            lock (_lock)
            {
                return _zones.Select(kv => new ZoneSnapshot(kv.Key, kv.Value.ServerId, kv.Value.SpawnPosition)).ToArray();
            }
        }

        /// <summary>TeleportService.TeleportToZone(Username, ZoneName, Position)</summary>
        public void TeleportToZone(string username, string zoneName, Vector3Data? position = null)
        {
            RequireZoneName(zoneName);
            lock (_lock)
            {
                if (!_zones.TryGetValue(zoneName, out var zone))
                    throw new InvalidOperationException($"No zone named '{zoneName}' — register it first.");
                Fire(username, zoneName, zone.ServerId, position ?? zone.SpawnPosition);
            }
        }

        /// <summary>TeleportService.TeleportToServer(Username, ServerId, Position) — null ServerId uses the default server.</summary>
        public void TeleportToServer(string username, string? serverId = null, Vector3Data? position = null)
        {
            lock (_lock)
            {
                Fire(username, serverId == null ? "default-server" : $"server:{serverId}", serverId, position);
            }
        }

        /// <summary>TeleportService.TeleportGroup(Usernames, ZoneName, Position)</summary>
        public void TeleportGroup(IEnumerable<string> usernames, string zoneName, Vector3Data? position = null)
        {
            foreach (string username in usernames ?? throw new ArgumentNullException(nameof(usernames)))
                TeleportToZone(username, zoneName, position);
        }

        /// <summary>TeleportService.OnTeleport(Username, Callback) — pass "*" to observe everyone.</summary>
        public void OnTeleport(string username, Action<TeleportEvent> callback)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));

            lock (_lock)
            {
                if (!_teleportCallbacks.TryGetValue(username, out var list))
                    _teleportCallbacks[username] = list = new List<Action<TeleportEvent>>();
                list.Add(callback);
            }
        }

        private void Fire(string username, string destination, string? serverId, Vector3Data? position)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));

            var @event = new TeleportEvent(username, destination, serverId, position);
            List<Action<TeleportEvent>>? specific;
            List<Action<TeleportEvent>>? wildcard;
            lock (_lock)
            {
                _teleportCallbacks.TryGetValue(username, out specific);
                _teleportCallbacks.TryGetValue("*", out wildcard);
            }
            specific?.ForEach(cb => cb(@event));
            wildcard?.ForEach(cb => cb(@event));
        }

        private static void RequireZoneName(string zoneName)
        {
            if (string.IsNullOrWhiteSpace(zoneName))
                throw new ArgumentException("ZoneName must not be empty.", nameof(zoneName));
        }
    }

    /// <summary>
    /// CameraService controls the camera view and screen shake for players
    /// ("*" applies to everyone).
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#cameraservice
    /// </summary>
    public class CameraService
    {
        /// <summary>Supported camera types.</summary>
        public static readonly IReadOnlyList<string> CameraTypes =
            new[] { "Classic", "Follow", "Fixed", "Scriptable" };

        /// <summary>Camera state snapshot for one player.</summary>
        public sealed record CameraSnapshot(
            string? CameraType, Vector3Data? Position, Vector3Data? LookAt,
            double? FieldOfView, string? LockedTo,
            double? ShakeIntensity, double? ShakeDuration);

        private readonly object _lock = new();
        private readonly Dictionary<string, CameraSnapshot> _states = new(StringComparer.Ordinal);

        /// <summary>CameraService.SetType(Username, CameraType)</summary>
        public void SetType(string username, string cameraType)
        {
            string? canonical = CameraTypes.FirstOrDefault(t =>
                string.Equals(t, cameraType, StringComparison.OrdinalIgnoreCase));
            if (canonical == null)
                throw new ArgumentException(
                    $"CameraType '{cameraType}' is not supported. Allowed: {string.Join(", ", CameraTypes)}.",
                    nameof(cameraType));

            Update(username, s => s with { CameraType = canonical });
        }

        /// <summary>CameraService.SetPosition(Username, Position, LookAt)</summary>
        public void SetPosition(string username, Vector3Data position, Vector3Data lookAt) =>
            Update(username, s => s with { Position = position, LookAt = lookAt });

        /// <summary>CameraService.SetFOV(Username, FieldOfView)</summary>
        public void SetFOV(string username, double fieldOfView)
        {
            if (fieldOfView <= 0 || fieldOfView > 180)
                throw new ArgumentException("FieldOfView must be in the (0, 180] range.", nameof(fieldOfView));
            Update(username, s => s with { FieldOfView = fieldOfView });
        }

        /// <summary>CameraService.LockTo(Username, Instance)</summary>
        public void LockTo(string username, string instanceName)
        {
            if (string.IsNullOrWhiteSpace(instanceName))
                throw new ArgumentException("Instance must not be empty.", nameof(instanceName));
            Update(username, s => s with { LockedTo = instanceName });
        }

        /// <summary>CameraService.Unlock(Username)</summary>
        public void Unlock(string username) => Update(username, s => s with { LockedTo = null });

        /// <summary>CameraService.Shake(Username, ShakeIntensity, Duration)</summary>
        public void Shake(string username, double shakeIntensity, double duration)
        {
            if (shakeIntensity < 0)
                throw new ArgumentException("ShakeIntensity must not be negative.", nameof(shakeIntensity));
            if (duration < 0)
                throw new ArgumentException("Duration must not be negative.", nameof(duration));
            Update(username, s => s with { ShakeIntensity = shakeIntensity, ShakeDuration = duration });
        }

        /// <summary>CameraService.Reset(Username)</summary>
        public void Reset(string username)
        {
            lock (_lock)
            {
                _states.Remove(username);
            }
        }

        /// <summary>Effective camera state for a player (specific overrides wildcard).</summary>
        public CameraSnapshot? GetState(string username)
        {
            lock (_lock)
            {
                if (_states.TryGetValue(username, out var specific))
                    return specific;
                return _states.TryGetValue("*", out var wildcard) ? wildcard : null;
            }
        }

        private void Update(string username, Func<CameraSnapshot, CameraSnapshot> update)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));

            lock (_lock)
            {
                var current = _states.TryGetValue(username, out var state)
                    ? state
                    : new CameraSnapshot(null, null, null, null, null, null, null);
                _states[username] = update(current);
            }
        }
    }
}
