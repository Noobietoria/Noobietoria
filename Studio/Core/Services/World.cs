using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// TweenService moves objects from one place to another smoothly.
    /// Target is a Part Name, or its GUID when names collide; an optional
    /// InstanceService is used to resolve the target and capture the start
    /// position (the actual interpolation runs in the engine renderer).
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#tweenservice
    /// </summary>
    public class TweenService
    {
        /// <summary>Playback state of a tween.</summary>
        public enum TweenState
        {
            Running,
            Paused,
            Stopped,
        }

        /// <summary>Tween snapshot (GetTween).</summary>
        public sealed record TweenSnapshot(
            string Target, Vector3Data Start, Vector3Data End, double Time,
            string? EasingDirection, string? EasingStyle, bool Loop, ColorData? RGBA,
            TweenState State, bool Reversed);

        private readonly object _lock = new();
        private readonly InstanceService? _instances;
        private readonly Dictionary<string, TweenSnapshot> _tweens = new(StringComparer.Ordinal);

        public TweenService(InstanceService? instances = null) => _instances = instances;

        /// <summary>
        /// TweenService.Move(Instance, EndPosition, Time, EasingDirection,
        /// EasingStyle, Loop, RGBA) — starts (or restarts) a tween.
        /// </summary>
        public void Move(
            string target,
            Vector3Data endPosition,
            double time,
            string? easingDirection = null,
            string? easingStyle = null,
            bool loop = false,
            ColorData? rgba = null)
        {
            if (string.IsNullOrWhiteSpace(target))
                throw new ArgumentException("Target must not be empty.", nameof(target));
            if (time <= 0)
                throw new ArgumentException("Time must be greater than 0.", nameof(time));

            lock (_lock)
            {
                _tweens[target] = new TweenSnapshot(
                    target, ResolveStart(target), endPosition, time,
                    easingDirection, easingStyle, loop, rgba, TweenState.Running, false);
            }
        }

        /// <summary>TweenService.Stop(Instance)</summary>
        public void Stop(string target) => Transition(target, TweenState.Stopped);

        /// <summary>TweenService.Resume(Instance) — continues a paused tween.</summary>
        public void Resume(string target) => Transition(target, TweenState.Running, requirePaused: true);

        /// <summary>TweenService.Pause(Instance)</summary>
        public void Pause(string target) => Transition(target, TweenState.Paused, requireRunning: true);

        /// <summary>
        /// TweenService.Back(Instance) — plays the tween backwards (swaps
        /// start/end and keeps the current state).
        /// </summary>
        public void Back(string target)
        {
            lock (_lock)
            {
                if (!_tweens.TryGetValue(RequireTarget(target), out var tween))
                    throw new InvalidOperationException($"No tween for '{target}' — call Move first.");
                _tweens[target] = tween with { Start = tween.End, End = tween.Start, Reversed = !tween.Reversed };
            }
        }

        /// <summary>Tween snapshot for a target, or null when no tween exists.</summary>
        public TweenSnapshot? GetTween(string target)
        {
            lock (_lock)
            {
                return _tweens.TryGetValue(target, out var tween) ? tween : null;
            }
        }

        private string RequireTarget(string target)
        {
            if (string.IsNullOrWhiteSpace(target))
                throw new ArgumentException("Target must not be empty.", nameof(target));
            return target;
        }

        private void Transition(string target, TweenState state, bool requirePaused = false, bool requireRunning = false)
        {
            RequireTarget(target);
            lock (_lock)
            {
                if (!_tweens.TryGetValue(target, out var tween))
                    throw new InvalidOperationException($"No tween for '{target}' — call Move first.");
                if (requirePaused && tween.State != TweenState.Paused)
                    throw new InvalidOperationException($"Tween for '{target}' is not paused.");
                if (requireRunning && tween.State != TweenState.Running)
                    throw new InvalidOperationException($"Tween for '{target}' is not running.");
                _tweens[target] = tween with { State = state };
            }
        }

        private Vector3Data ResolveStart(string target)
        {
            if (_instances == null)
                return new Vector3Data(0, 0, 0);

            var instance = _instances.GetByGUID(target)
                ?? _instances.GetByGUID(_instances.GetGUID(target) ?? string.Empty);
            if (instance == null)
                throw new InvalidOperationException($"No Instance found for tween target '{target}'.");

            return instance.GetProperty("Position", new Vector3Data(0, 0, 0));
        }
    }

    /// <summary>
    /// LightingService (also known as EnvironmentService) adjusts the time
    /// of day, ambient lighting and weather.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#lightingservice
    /// </summary>
    public class LightingService
    {
        /// <summary>Supported weather types.</summary>
        public static readonly IReadOnlyList<string> WeatherTypes =
            new[] { "Clear", "Rain", "Storm", "Fog", "Snow" };

        /// <summary>Weather snapshot (GetWeather).</summary>
        public sealed record WeatherState(string Type, double Intensity);

        /// <summary>Fog configuration (SetFog).</summary>
        public sealed record FogState(bool Enabled, ColorData? Color, double? Start, double? End);

        private readonly object _lock = new();
        private TimeSpan _timeOfDay = new(12, 0, 0);
        private WeatherState _weather = new("Clear", 1.0);
        private FogState _fog = new(false, null, null, null);
        private readonly Dictionary<string, object?> _properties = new(StringComparer.Ordinal);

        /// <summary>LightingService.SetTime(TimeOfDay) — "HH:MM:SS".</summary>
        public void SetTime(string timeOfDay)
        {
            if (!TimeSpan.TryParseExact(timeOfDay, @"hh\:mm\:ss", CultureInfo.InvariantCulture, out var parsed))
                throw new ArgumentException(
                    "TimeOfDay must use the \"HH:MM:SS\" format (e.g. \"14:30:00\").", nameof(timeOfDay));

            lock (_lock)
            {
                _timeOfDay = parsed;
            }
        }

        /// <summary>LightingService.GetTime() — "HH:MM:SS".</summary>
        public string GetTime()
        {
            lock (_lock)
            {
                return _timeOfDay.ToString(@"hh\:mm\:ss", CultureInfo.InvariantCulture);
            }
        }

        /// <summary>LightingService.SetWeather(WeatherType, Intensity)</summary>
        public void SetWeather(string weatherType, double intensity = 1.0)
        {
            string? canonical = WeatherTypes.FirstOrDefault(t =>
                string.Equals(t, weatherType, StringComparison.OrdinalIgnoreCase));
            if (canonical == null)
                throw new ArgumentException(
                    $"WeatherType '{weatherType}' is not supported. Allowed: {string.Join(", ", WeatherTypes)}.",
                    nameof(weatherType));
            if (intensity is < 0 or > 1)
                throw new ArgumentException("Intensity must be between 0 and 1.", nameof(intensity));

            lock (_lock)
            {
                _weather = new WeatherState(canonical, intensity);
            }
        }

        /// <summary>LightingService.GetWeather()</summary>
        public WeatherState GetWeather()
        {
            lock (_lock)
            {
                return _weather;
            }
        }

        /// <summary>LightingService.SetProperty(Property, Value)</summary>
        public void SetProperty(string property, object? value)
        {
            if (string.IsNullOrWhiteSpace(property))
                throw new ArgumentException("Property must not be empty.", nameof(property));
            lock (_lock)
            {
                _properties[property] = value;
            }
        }

        /// <summary>LightingService.GetProperty(Property)</summary>
        public object? GetProperty(string property)
        {
            lock (_lock)
            {
                return _properties.TryGetValue(property, out var value) ? value : null;
            }
        }

        /// <summary>LightingService.SetAmbient(RGBA)</summary>
        public void SetAmbient(ColorData rgba) => SetProperty("Ambient", rgba);

        /// <summary>LightingService.SetFog(Enabled, Color, Start, End)</summary>
        public void SetFog(bool enabled, ColorData? color = null, double? start = null, double? end = null)
        {
            lock (_lock)
            {
                _fog = new FogState(enabled, color, start, end);
                _properties["Fog"] = _fog;
            }
        }
    }

    /// <summary>
    /// CollisionService manages collision groups: parts are assigned to
    /// groups and collisions between two groups are on/off (default on).
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#collisionservice
    /// </summary>
    public class CollisionService
    {
        private readonly object _lock = new();
        private readonly HashSet<string> _groups = new(StringComparer.Ordinal);
        private readonly Dictionary<string, string> _assignment = new(StringComparer.Ordinal);
        private readonly HashSet<(string A, string B)> _disabledPairs = new();

        /// <summary>CollisionService.CreateGroup(GroupName)</summary>
        public void CreateGroup(string groupName)
        {
            RequireGroupName(groupName);
            lock (_lock)
            {
                if (!_groups.Add(groupName))
                    throw new InvalidOperationException($"Collision group '{groupName}' already exists.");
            }
        }

        /// <summary>CollisionService.DeleteGroup(GroupName)</summary>
        public void DeleteGroup(string groupName)
        {
            RequireGroupName(groupName);
            lock (_lock)
            {
                if (!_groups.Remove(groupName))
                    throw new InvalidOperationException($"No collision group named '{groupName}'.");
                _disabledPairs.RemoveWhere(pair => pair.A == groupName || pair.B == groupName);
                foreach (string instance in _assignment.Where(kv => kv.Value == groupName).Select(kv => kv.Key).ToArray())
                    _assignment.Remove(instance);
            }
        }

        /// <summary>CollisionService.AssignGroup(Instance, GroupName)</summary>
        public void AssignGroup(string instanceName, string groupName)
        {
            if (string.IsNullOrWhiteSpace(instanceName))
                throw new ArgumentException("Instance must not be empty.", nameof(instanceName));
            RequireGroupName(groupName);
            lock (_lock)
            {
                if (!_groups.Contains(groupName))
                    throw new InvalidOperationException($"No collision group named '{groupName}'.");
                _assignment[instanceName] = groupName;
            }
        }

        /// <summary>CollisionService.SetCollision(GroupA, GroupB, Boolean)</summary>
        public void SetCollision(string groupA, string groupB, bool enabled)
        {
            RequireGroupName(groupA);
            RequireGroupName(groupB);
            lock (_lock)
            {
                if (!_groups.Contains(groupA) || !_groups.Contains(groupB))
                    throw new InvalidOperationException("Both groups must exist before SetCollision.");
                var pair = Pair(groupA, groupB);
                if (enabled)
                    _disabledPairs.Remove(pair);
                else
                    _disabledPairs.Add(pair);
            }
        }

        /// <summary>CollisionService.GetGroup(Instance) — group name or null.</summary>
        public string? GetGroup(string instanceName)
        {
            lock (_lock)
            {
                return _assignment.TryGetValue(instanceName, out var group) ? group : null;
            }
        }

        /// <summary>CollisionService.GetGroups()</summary>
        public IReadOnlyList<string> GetGroups()
        {
            lock (_lock)
            {
                return _groups.OrderBy(g => g, StringComparer.Ordinal).ToArray();
            }
        }

        /// <summary>Whether two groups currently collide (default: true).</summary>
        public bool Collides(string groupA, string groupB)
        {
            lock (_lock)
            {
                return !_disabledPairs.Contains(Pair(groupA, groupB));
            }
        }

        private static (string A, string B) Pair(string a, string b) =>
            string.CompareOrdinal(a, b) <= 0 ? (a, b) : (b, a);

        private static void RequireGroupName(string groupName)
        {
            if (string.IsNullOrWhiteSpace(groupName))
                throw new ArgumentException("GroupName must not be empty.", nameof(groupName));
        }
    }

    /// <summary>
    /// PhysicsService controls global gravity plus per-instance forces,
    /// velocity and physical properties. v0 keeps the state for the engine
    /// physics step to consume; it does not integrate motion itself.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#physicsservice
    /// </summary>
    public class PhysicsService
    {
        /// <summary>Default gravity in studs/s².</summary>
        public const double DefaultGravity = 196.2;

        private readonly object _lock = new();
        private double _gravity = DefaultGravity;
        private readonly Dictionary<string, PhysicsBody> _bodies = new(StringComparer.Ordinal);

        /// <summary>Per-instance physical state (GetVelocity and friends).</summary>
        public sealed record PhysicsSnapshot(
            Vector3Data Velocity, bool Anchored, double? Mass, double? Friction, double? Elasticity);

        /// <summary>PhysicsService.SetGravity(Gravity)</summary>
        public void SetGravity(double gravity)
        {
            if (double.IsNaN(gravity) || double.IsInfinity(gravity))
                throw new ArgumentException("Gravity must be a finite number.", nameof(gravity));
            lock (_lock)
            {
                _gravity = gravity;
            }
        }

        /// <summary>PhysicsService.GetGravity()</summary>
        public double GetGravity()
        {
            lock (_lock)
            {
                return _gravity;
            }
        }

        /// <summary>PhysicsService.ApplyForce(Instance, Force, Duration)</summary>
        public void ApplyForce(string instanceName, Vector3Data force, double? duration = null)
        {
            RequireInstance(instanceName);
            if (duration.HasValue && duration.Value < 0)
                throw new ArgumentException("Duration must not be negative.", nameof(duration));
            lock (_lock)
            {
                Body(instanceName).Forces.Add((force, duration));
            }
        }

        /// <summary>PhysicsService.ApplyImpulse(Instance, Force)</summary>
        public void ApplyImpulse(string instanceName, Vector3Data force)
        {
            RequireInstance(instanceName);
            lock (_lock)
            {
                var body = Body(instanceName);
                var velocity = body.Velocity;
                body.Velocity = new Vector3Data(
                    velocity.X + force.X, velocity.Y + force.Y, velocity.Z + force.Z);
            }
        }

        /// <summary>PhysicsService.SetVelocity(Instance, Velocity)</summary>
        public void SetVelocity(string instanceName, Vector3Data velocity)
        {
            RequireInstance(instanceName);
            lock (_lock)
            {
                Body(instanceName).Velocity = velocity;
            }
        }

        /// <summary>PhysicsService.GetVelocity(Instance)</summary>
        public Vector3Data GetVelocity(string instanceName)
        {
            RequireInstance(instanceName);
            lock (_lock)
            {
                return Body(instanceName).Velocity;
            }
        }

        /// <summary>PhysicsService.SetAnchored(Instance, Boolean)</summary>
        public void SetAnchored(string instanceName, bool anchored)
        {
            RequireInstance(instanceName);
            lock (_lock)
            {
                Body(instanceName).Anchored = anchored;
            }
        }

        /// <summary>PhysicsService.SetMass(Instance, Mass)</summary>
        public void SetMass(string instanceName, double mass)
        {
            RequirePositive(mass, nameof(mass));
            lock (_lock)
            {
                Body(instanceName).Mass = mass;
            }
        }

        /// <summary>PhysicsService.SetFriction(Instance, Friction)</summary>
        public void SetFriction(string instanceName, double friction)
        {
            RequirePositive(friction, nameof(friction));
            lock (_lock)
            {
                Body(instanceName).Friction = friction;
            }
        }

        /// <summary>PhysicsService.SetElasticity(Instance, Elasticity)</summary>
        public void SetElasticity(string instanceName, double elasticity)
        {
            RequirePositive(elasticity, nameof(elasticity));
            lock (_lock)
            {
                Body(instanceName).Elasticity = elasticity;
            }
        }

        /// <summary>Snapshot of an instance's physics state (for tests/tooling).</summary>
        public PhysicsSnapshot GetSnapshot(string instanceName)
        {
            RequireInstance(instanceName);
            lock (_lock)
            {
                var body = Body(instanceName);
                return new PhysicsSnapshot(body.Velocity, body.Anchored, body.Mass, body.Friction, body.Elasticity);
            }
        }

        private sealed class PhysicsBody
        {
            public Vector3Data Velocity;
            public bool Anchored;
            public double? Mass;
            public double? Friction;
            public double? Elasticity;
            public List<(Vector3Data Force, double? Duration)> Forces = new();
        }

        private PhysicsBody Body(string instanceName)
        {
            if (!_bodies.TryGetValue(instanceName, out var body))
                _bodies[instanceName] = body = new PhysicsBody();
            return body;
        }

        private static void RequireInstance(string instanceName)
        {
            if (string.IsNullOrWhiteSpace(instanceName))
                throw new ArgumentException("Instance must not be empty.", nameof(instanceName));
        }

        private static void RequirePositive(double value, string paramName)
        {
            if (value < 0)
                throw new ArgumentException("Value must not be negative.", paramName);
        }
    }
}
