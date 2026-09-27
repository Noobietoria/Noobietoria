using System;
using System.Collections.Generic;
using System.Linq;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// EffectService creates visual/particle effects at a position or
    /// attached to an instance, plus screen effects for players.
    /// v0 records the emitted effects; the engine renderer consumes them.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#effectservice
    /// </summary>
    public class EffectService
    {
        /// <summary>One active effect run.</summary>
        public sealed record ActiveEffect(
            string EffectId, Vector3Data? Position, string? Instance,
            IReadOnlyDictionary<string, object?> Properties);

        /// <summary>One active screen effect for a player ("*" = everyone).</summary>
        public sealed record ScreenEffectRun(
            string Username, string EffectType, IReadOnlyDictionary<string, object?> Properties, double Duration);

        private readonly object _lock = new();
        private readonly List<ActiveEffect> _active = new();
        private readonly Dictionary<string, List<ScreenEffectRun>> _screen = new(StringComparer.Ordinal);

        /// <summary>EffectService.Emit(EffectId, Position, Properties)</summary>
        public void Emit(string effectId, Vector3Data position, IReadOnlyDictionary<string, object?>? properties = null)
        {
            if (string.IsNullOrWhiteSpace(effectId))
                throw new ArgumentException("EffectId must not be empty.", nameof(effectId));

            lock (_lock)
            {
                _active.Add(new ActiveEffect(effectId, position, null, Copy(properties)));
            }
        }

        /// <summary>EffectService.EmitOn(EffectId, Instance, Properties)</summary>
        public void EmitOn(string effectId, string instanceName, IReadOnlyDictionary<string, object?>? properties = null)
        {
            if (string.IsNullOrWhiteSpace(effectId))
                throw new ArgumentException("EffectId must not be empty.", nameof(effectId));
            if (string.IsNullOrWhiteSpace(instanceName))
                throw new ArgumentException("Instance must not be empty.", nameof(instanceName));

            lock (_lock)
            {
                _active.Add(new ActiveEffect(effectId, null, instanceName, Copy(properties)));
            }
        }

        /// <summary>EffectService.Stop(EffectId) — stops every run of the effect.</summary>
        public void Stop(string effectId)
        {
            if (string.IsNullOrWhiteSpace(effectId))
                throw new ArgumentException("EffectId must not be empty.", nameof(effectId));
            lock (_lock)
            {
                _active.RemoveAll(e => e.EffectId == effectId);
            }
        }

        /// <summary>EffectService.StopAll()</summary>
        public void StopAll()
        {
            lock (_lock)
            {
                _active.Clear();
            }
        }

        /// <summary>EffectService.ScreenEffect(Username, EffectType, Properties, Duration) — "*" applies to everyone.</summary>
        public void ScreenEffect(
            string username, string effectType,
            IReadOnlyDictionary<string, object?>? properties = null, double duration = 0)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (string.IsNullOrWhiteSpace(effectType))
                throw new ArgumentException("EffectType must not be empty.", nameof(effectType));
            if (duration < 0)
                throw new ArgumentException("Duration must not be negative.", nameof(duration));

            lock (_lock)
            {
                if (!_screen.TryGetValue(username, out var list))
                    _screen[username] = list = new List<ScreenEffectRun>();
                list.Add(new ScreenEffectRun(username, effectType, Copy(properties), duration));
            }
        }

        /// <summary>EffectService.ClearScreenEffect(Username)</summary>
        public void ClearScreenEffect(string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            lock (_lock)
            {
                _screen.Remove(username);
            }
        }

        /// <summary>Active world effects (engine consumption / tests).</summary>
        public IReadOnlyList<ActiveEffect> GetActiveEffects()
        {
            lock (_lock)
            {
                return _active.ToArray();
            }
        }

        /// <summary>Active screen effects for one player.</summary>
        public IReadOnlyList<ScreenEffectRun> GetScreenEffects(string username)
        {
            lock (_lock)
            {
                return _screen.TryGetValue(username, out var list) ? list.ToArray() : Array.Empty<ScreenEffectRun>();
            }
        }

        private static IReadOnlyDictionary<string, object?> Copy(IReadOnlyDictionary<string, object?>? source)
        {
            if (source == null)
                return new Dictionary<string, object?>();
            return new Dictionary<string, object?>(source);
        }
    }

    /// <summary>
    /// SoundEffectService plays 3D sound effects at world positions or on
    /// instances, with volume (0.0–1.0) and rolloff distance in studs.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#soundeffectservice
    /// </summary>
    public class SoundEffectService
    {
        /// <summary>One active (playing) sound.</summary>
        public sealed record ActiveSound(
            string SoundId, Vector3Data? Position, string? Instance, double Volume, double? RolloffDistance);

        private readonly object _lock = new();
        private readonly List<ActiveSound> _active = new();
        private readonly HashSet<string> _preloaded = new(StringComparer.Ordinal);

        /// <summary>SoundEffectService.Play(SoundId, Position, Volume, RolloffDistance)</summary>
        public void Play(string soundId, Vector3Data position, double volume = 1.0, double? rolloffDistance = null)
        {
            RequireVolume(volume);
            if (rolloffDistance.HasValue && rolloffDistance.Value < 0)
                throw new ArgumentException("RolloffDistance must not be negative.", nameof(rolloffDistance));

            lock (_lock)
            {
                ReplaceActive(new ActiveSound(RequireSoundId(soundId), position, null, volume, rolloffDistance));
            }
        }

        /// <summary>SoundEffectService.PlayOn(SoundId, Instance, Volume, RolloffDistance)</summary>
        public void PlayOn(string soundId, string instanceName, double volume = 1.0, double? rolloffDistance = null)
        {
            RequireVolume(volume);
            if (string.IsNullOrWhiteSpace(instanceName))
                throw new ArgumentException("Instance must not be empty.", nameof(instanceName));

            lock (_lock)
            {
                ReplaceActive(new ActiveSound(RequireSoundId(soundId), null, instanceName, volume, rolloffDistance));
            }
        }

        /// <summary>SoundEffectService.Stop(SoundId)</summary>
        public void Stop(string soundId)
        {
            RequireSoundId(soundId);
            lock (_lock)
            {
                _active.RemoveAll(s => s.SoundId == soundId);
            }
        }

        /// <summary>SoundEffectService.SetVolume(SoundId, Volume) — updates every active run of the sound.</summary>
        public void SetVolume(string soundId, double volume)
        {
            RequireSoundId(soundId);
            RequireVolume(volume);
            lock (_lock)
            {
                if (!_active.Any(s => s.SoundId == soundId))
                    throw new InvalidOperationException($"Sound '{soundId}' is not playing.");
                for (int i = 0; i < _active.Count; i++)
                {
                    if (_active[i].SoundId == soundId)
                        _active[i] = _active[i] with { Volume = volume };
                }
            }
        }

        /// <summary>SoundEffectService.Preload(SoundId)</summary>
        public void Preload(string soundId)
        {
            RequireSoundId(soundId);
            lock (_lock)
            {
                _preloaded.Add(soundId);
            }
        }

        /// <summary>SoundEffectService.Release(SoundId)</summary>
        public void Release(string soundId)
        {
            RequireSoundId(soundId);
            lock (_lock)
            {
                _preloaded.Remove(soundId);
                _active.RemoveAll(s => s.SoundId == soundId);
            }
        }

        /// <summary>Active sounds (engine consumption / tests).</summary>
        public IReadOnlyList<ActiveSound> GetActiveSounds()
        {
            lock (_lock)
            {
                return _active.ToArray();
            }
        }

        /// <summary>Whether a sound id has been preloaded.</summary>
        public bool IsPreloaded(string soundId)
        {
            RequireSoundId(soundId);
            lock (_lock)
            {
                return _preloaded.Contains(soundId);
            }
        }

        private void ReplaceActive(ActiveSound sound)
        {
            _active.RemoveAll(s => s.SoundId == sound.SoundId);
            _active.Add(sound);
        }

        private static void RequireVolume(double volume)
        {
            if (volume is < 0.0 or > 1.0)
                throw new ArgumentException("Volume must be between 0.0 and 1.0.", nameof(volume));
        }

        private static string RequireSoundId(string soundId)
        {
            if (string.IsNullOrWhiteSpace(soundId))
                throw new ArgumentException("SoundId must not be empty.", nameof(soundId));
            return soundId;
        }
    }

    /// <summary>
    /// ProximityService creates interaction prompts attached to a Part/Model
    /// with MaxDistance, an activation key (default "E") and HoldDuration
    /// (0 = instant).
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#proximityservice
    /// </summary>
    public class ProximityService
    {
        /// <summary>Prompt snapshot (GetPrompt / GetAllPrompts).</summary>
        public sealed record PromptSnapshot(
            string PromptId, string Instance, string Label, string KeyCode,
            double MaxDistance, double HoldDuration, bool Visible, bool Enabled);

        private readonly object _lock = new();
        private readonly Dictionary<string, PromptSnapshot> _prompts = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action>> _callbacks = new(StringComparer.Ordinal);

        /// <summary>
        /// ProximityService.Create(PromptId, Instance, Label, KeyCode,
        /// MaxDistance, HoldDuration)
        /// </summary>
        public void Create(
            string promptId, string instanceName, string label = "",
            string keyCode = "E", double maxDistance = 10, double holdDuration = 0)
        {
            if (string.IsNullOrWhiteSpace(promptId))
                throw new ArgumentException("PromptId must not be empty.", nameof(promptId));
            if (string.IsNullOrWhiteSpace(instanceName))
                throw new ArgumentException("Instance must not be empty.", nameof(instanceName));
            if (maxDistance < 0)
                throw new ArgumentException("MaxDistance must not be negative.", nameof(maxDistance));
            if (holdDuration < 0)
                throw new ArgumentException("HoldDuration must not be negative.", nameof(holdDuration));

            lock (_lock)
            {
                if (!_prompts.TryAdd(promptId,
                        new PromptSnapshot(promptId, instanceName, label, keyCode, maxDistance, holdDuration, true, true)))
                    throw new InvalidOperationException($"Prompt '{promptId}' already exists.");
            }
        }

        /// <summary>ProximityService.Remove(PromptId)</summary>
        public void Remove(string promptId)
        {
            lock (_lock)
            {
                if (!_prompts.Remove(promptId))
                    throw new InvalidOperationException($"No prompt '{promptId}'.");
                _callbacks.Remove(promptId);
            }
        }

        /// <summary>ProximityService.SetVisible(PromptId, Boolean)</summary>
        public void SetVisible(string promptId, bool visible) =>
            Update(promptId, p => p with { Visible = visible });

        /// <summary>ProximityService.SetEnabled(PromptId, Boolean)</summary>
        public void SetEnabled(string promptId, bool enabled) =>
            Update(promptId, p => p with { Enabled = enabled });

        /// <summary>ProximityService.GetPrompt(PromptId)</summary>
        public PromptSnapshot? GetPrompt(string promptId)
        {
            lock (_lock)
            {
                return _prompts.TryGetValue(promptId, out var prompt) ? prompt : null;
            }
        }

        /// <summary>ProximityService.GetAllPrompts()</summary>
        public IReadOnlyList<PromptSnapshot> GetAllPrompts()
        {
            lock (_lock)
            {
                return _prompts.Values.ToArray();
            }
        }

        /// <summary>ProximityService.OnTriggered(OnEnter/OnExit follow the same pattern).</summary>
        public void OnTriggered(string promptId, Action callback) => AddCallback(promptId, "Triggered", callback);

        /// <summary>ProximityService.OnEnter(PromptId, Callback)</summary>
        public void OnEnter(string promptId, Action callback) => AddCallback(promptId, "Enter", callback);

        /// <summary>ProximityService.OnExit(PromptId, Callback)</summary>
        public void OnExit(string promptId, Action callback) => AddCallback(promptId, "Exit", callback);

        /// <summary>Registered callbacks of one kind ("Triggered"/"Enter"/"Exit") — used by the engine and tests.</summary>
        public IReadOnlyList<Action> GetCallbacks(string promptId, string kind)
        {
            lock (_lock)
            {
                return _callbacks.TryGetValue(Key(promptId, kind), out var list)
                    ? list.ToArray()
                    : Array.Empty<Action>();
            }
        }

        private void Update(string promptId, Func<PromptSnapshot, PromptSnapshot> update)
        {
            if (string.IsNullOrWhiteSpace(promptId))
                throw new ArgumentException("PromptId must not be empty.", nameof(promptId));
            lock (_lock)
            {
                if (!_prompts.TryGetValue(promptId, out var prompt))
                    throw new InvalidOperationException($"No prompt '{promptId}'.");
                _prompts[promptId] = update(prompt);
            }
        }

        private void AddCallback(string promptId, string kind, Action callback)
        {
            if (string.IsNullOrWhiteSpace(promptId))
                throw new ArgumentException("PromptId must not be empty.", nameof(promptId));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));

            lock (_lock)
            {
                string key = Key(promptId, kind);
                if (!_callbacks.TryGetValue(key, out var list))
                    _callbacks[key] = list = new List<Action>();
                list.Add(callback);
            }
        }

        private static string Key(string promptId, string kind) => promptId + "\u0001" + kind;
    }

    /// <summary>
    /// PathfindingService computes paths through the 3D space and moves NPCs
    /// along them. v0 generates straight-line waypoints (the real navigation
    /// mesh plugs in later); Destination may be coordinates or the name of
    /// an Instance (resolved through the optional InstanceService).
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#pathfindingservice
    /// </summary>
    public class PathfindingService
    {
        /// <summary>Navigation state of an NPC.</summary>
        public enum NavigationState
        {
            Idle,
            Moving,
            Paused,
            Stopped,
        }

        /// <summary>Navigation snapshot (GetWaypoints).</summary>
        public sealed record NavigationSnapshot(string NpcName, NavigationState State, IReadOnlyList<Vector3Data> Waypoints);

        private const double WaypointSpacing = 5.0;

        private readonly object _lock = new();
        private readonly InstanceService? _instances;
        private readonly Dictionary<string, NavigationSnapshot> _navigation = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action>> _reached = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action>> _blocked = new(StringComparer.Ordinal);

        public PathfindingService(InstanceService? instances = null) => _instances = instances;

        /// <summary>PathfindingService.ComputePath(NpcName, Destination, AgentRadius, AgentHeight)</summary>
        public IReadOnlyList<Vector3Data> ComputePath(
            string npcName, Vector3Data destination, double agentRadius = 2.0, double agentHeight = 5.0)
        {
            RequireAgent(npcName, agentRadius, agentHeight);
            var waypoints = BuildWaypoints(new Vector3Data(0, 0, 0), destination);
            Store(npcName, NavigationState.Idle, waypoints);
            return waypoints;
        }

        /// <summary>ComputePath overload taking an Instance name as destination.</summary>
        public IReadOnlyList<Vector3Data> ComputePath(
            string npcName, string destinationInstance, double agentRadius = 2.0, double agentHeight = 5.0)
        {
            RequireAgent(npcName, agentRadius, agentHeight);
            var destination = ResolveInstancePosition(destinationInstance);
            return ComputePath(npcName, destination, agentRadius, agentHeight);
        }

        /// <summary>PathfindingService.MoveTo(NpcName, Destination) — computes a path and starts moving.</summary>
        public void MoveTo(string npcName, Vector3Data destination)
        {
            ComputePath(npcName, destination);
            lock (_lock)
            {
                _navigation[npcName] = _navigation[npcName] with { State = NavigationState.Moving };
            }
        }

        /// <summary>PathfindingService.MoveAlongWaypoints(NpcName, Waypoints)</summary>
        public void MoveAlongWaypoints(string npcName, IReadOnlyList<Vector3Data> waypoints)
        {
            if (string.IsNullOrWhiteSpace(npcName))
                throw new ArgumentException("NpcName must not be empty.", nameof(npcName));
            if (waypoints == null || waypoints.Count == 0)
                throw new ArgumentException("Waypoints must not be empty.", nameof(waypoints));

            lock (_lock)
            {
                Store(npcName, NavigationState.Moving, waypoints);
            }
        }

        /// <summary>PathfindingService.Stop(NpcName)</summary>
        public void Stop(string npcName) => Transition(npcName, NavigationState.Stopped);

        /// <summary>PathfindingService.Pause(NpcName)</summary>
        public void Pause(string npcName) => Transition(npcName, NavigationState.Paused, requireMoving: true);

        /// <summary>PathfindingService.Resume(NpcName)</summary>
        public void Resume(string npcName) => Transition(npcName, NavigationState.Moving, requirePaused: true);

        /// <summary>PathfindingService.GetWaypoints(NpcName)</summary>
        public IReadOnlyList<Vector3Data> GetWaypoints(string npcName)
        {
            if (string.IsNullOrWhiteSpace(npcName))
                throw new ArgumentException("NpcName must not be empty.", nameof(npcName));
            lock (_lock)
            {
                return _navigation.TryGetValue(npcName, out var nav) ? nav.Waypoints : Array.Empty<Vector3Data>();
            }
        }

        /// <summary>Navigation state of an NPC.</summary>
        public NavigationState GetState(string npcName)
        {
            if (string.IsNullOrWhiteSpace(npcName))
                throw new ArgumentException("NpcName must not be empty.", nameof(npcName));
            lock (_lock)
            {
                return _navigation.TryGetValue(npcName, out var nav) ? nav.State : NavigationState.Idle;
            }
        }

        /// <summary>PathfindingService.OnReached(NpcName, Callback)</summary>
        public void OnReached(string npcName, Action callback) => AddCallback(_reached, npcName, callback);

        /// <summary>PathfindingService.OnBlocked(NpcName, Callback)</summary>
        public void OnBlocked(string npcName, Action callback) => AddCallback(_blocked, npcName, callback);

        /// <summary>Registered callbacks per NPC — consumed by the navigation engine and tests.</summary>
        internal IReadOnlyList<Action> ReachedCallbacks(string npcName) => CallbacksOf(_reached, npcName);

        internal IReadOnlyList<Action> BlockedCallbacks(string npcName) => CallbacksOf(_blocked, npcName);

        private Vector3Data[] BuildWaypoints(Vector3Data start, Vector3Data destination)
        {
            double dx = destination.X - start.X;
            double dy = destination.Y - start.Y;
            double dz = destination.Z - start.Z;
            double distance = Math.Sqrt(dx * dx + dy * dy + dz * dz);
            int segments = Math.Max(1, (int)Math.Ceiling(distance / WaypointSpacing));

            var waypoints = new List<Vector3Data>(segments);
            for (int i = 1; i <= segments; i++)
            {
                double t = (double)i / segments;
                waypoints.Add(new Vector3Data(
                    start.X + dx * t, start.Y + dy * t, start.Z + dz * t));
            }
            return waypoints.ToArray();
        }

        private Vector3Data ResolveInstancePosition(string instanceName)
        {
            if (_instances == null || string.IsNullOrWhiteSpace(instanceName))
                throw new ArgumentException(
                    "Destination must be coordinates, or an Instance name with an InstanceService attached.",
                    nameof(instanceName));

            var instance = _instances.GetByGUID(instanceName)
                ?? _instances.GetByGUID(_instances.GetGUID(instanceName) ?? string.Empty)
                ?? throw new InvalidOperationException($"No Instance named '{instanceName}' to navigate to.");
            return instance.GetProperty("Position", new Vector3Data(0, 0, 0));
        }

        private void Store(string npcName, NavigationState state, IReadOnlyList<Vector3Data> waypoints)
        {
            lock (_lock)
            {
                _navigation[npcName] = new NavigationSnapshot(npcName, state, waypoints);
            }
        }

        private void Transition(string npcName, NavigationState state, bool requireMoving = false, bool requirePaused = false)
        {
            if (string.IsNullOrWhiteSpace(npcName))
                throw new ArgumentException("NpcName must not be empty.", nameof(npcName));
            lock (_lock)
            {
                if (!_navigation.TryGetValue(npcName, out var nav))
                    throw new InvalidOperationException($"No navigation for '{npcName}' — compute a path first.");
                if (requireMoving && nav.State != NavigationState.Moving)
                    throw new InvalidOperationException($"'{npcName}' is not moving.");
                if (requirePaused && nav.State != NavigationState.Paused)
                    throw new InvalidOperationException($"'{npcName}' is not paused.");
                _navigation[npcName] = nav with { State = state };
            }
        }

        private static void RequireAgent(string npcName, double agentRadius, double agentHeight)
        {
            if (string.IsNullOrWhiteSpace(npcName))
                throw new ArgumentException("NpcName must not be empty.", nameof(npcName));
            if (agentRadius <= 0)
                throw new ArgumentException("AgentRadius must be greater than 0.", nameof(agentRadius));
            if (agentHeight <= 0)
                throw new ArgumentException("AgentHeight must be greater than 0.", nameof(agentHeight));
        }

        private static void AddCallback(Dictionary<string, List<Action>> map, string npcName, Action callback)
        {
            if (string.IsNullOrWhiteSpace(npcName))
                throw new ArgumentException("NpcName must not be empty.", nameof(npcName));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (map)
            {
                if (!map.TryGetValue(npcName, out var list))
                    map[npcName] = list = new List<Action>();
                list.Add(callback);
            }
        }

        private static IReadOnlyList<Action> CallbacksOf(Dictionary<string, List<Action>> map, string npcName)
        {
            lock (map)
            {
                return map.TryGetValue(npcName, out var list) ? list.ToArray() : Array.Empty<Action>();
            }
        }
    }
}
