using System;
using System.Collections.Generic;
using System.Linq;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// AccessoryService equips marketplace accessories on a Player or NPC in
    /// one call — the static-variable variant of the Accessory instance that
    /// cannot be overridden from Luau. All three parameters are strings and
    /// must not be nil; the ID string is comma-separated Accessory IDs.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#accessoryservice
    /// </summary>
    public class AccessoryService
    {
        private readonly object _lock = new();
        private readonly Dictionary<string, List<string>> _equipped = new(StringComparer.Ordinal);

        /// <summary>
        /// AccessoryService(TargetName, TargetType, Ids) — equips the listed
        /// accessories on a "Player" or "NPC". The ID string is
        /// comma-separated (whitespace tolerated), e.g.
        /// " 1082345 , 1082350, 1082360".
        /// </summary>
        public void Equip(string targetName, string targetType, string idString)
        {
            if (string.IsNullOrWhiteSpace(targetName))
                throw new ArgumentException("TargetName must not be empty.", nameof(targetName));
            if (string.IsNullOrWhiteSpace(targetType))
                throw new ArgumentException("TargetType must not be empty.", nameof(targetType));
            if (string.IsNullOrWhiteSpace(idString))
                throw new ArgumentException(
                    "IdString must not be empty (comma-separated Accessory IDs).", nameof(idString));
            if (!string.Equals(targetType, "Player", StringComparison.OrdinalIgnoreCase)
                && !string.Equals(targetType, "NPC", StringComparison.OrdinalIgnoreCase))
                throw new ArgumentException(
                    $"TargetType '{targetType}' is not supported (use \"Player\" or \"NPC\").", nameof(targetType));

            var ids = idString
                .Split(',', StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries)
                .ToArray();
            if (ids.Length == 0)
                throw new ArgumentException("IdString must contain at least one Accessory ID.", nameof(idString));

            lock (_lock)
            {
                _equipped[targetName] = ids.ToList();
            }
        }

        /// <summary>Equipped accessory IDs of a target, in given order.</summary>
        public IReadOnlyList<string> GetEquipped(string targetName)
        {
            if (string.IsNullOrWhiteSpace(targetName))
                throw new ArgumentException("TargetName must not be empty.", nameof(targetName));
            lock (_lock)
            {
                return _equipped.GetValueOrDefault(targetName)?.ToArray() ?? Array.Empty<string>();
            }
        }
    }

    /// <summary>
    /// AnalyticsService tracks events, errors and counters with optional
    /// player attribution and properties.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#analyticsservice
    /// </summary>
    public class AnalyticsService
    {
        /// <summary>One recorded event (GetEvents payload).</summary>
        public sealed record AnalyticsEvent(string EventName, string? Username,
            IReadOnlyDictionary<string, object?> Properties, DateTime At);

        private readonly object _lock = new();
        private readonly Func<DateTime> _clock;
        private readonly List<AnalyticsEvent> _events = new();
        private readonly Dictionary<string, double> _counters = new(StringComparer.Ordinal);

        public AnalyticsService(Func<DateTime>? clock = null) => _clock = clock ?? (() => DateTime.UtcNow);

        /// <summary>AnalyticsService.TrackEvent(EventName, Username, Properties)</summary>
        public void TrackEvent(string eventName, string? username = null,
            IReadOnlyDictionary<string, object?>? properties = null)
        {
            Add("Event", eventName, username, properties);
        }

        /// <summary>AnalyticsService.TrackError(ErrorMessage, Username, Properties)</summary>
        public void TrackError(string errorMessage, string? username = null,
            IReadOnlyDictionary<string, object?>? properties = null)
        {
            Add("Error", errorMessage, username, properties);
        }

        /// <summary>AnalyticsService.SetCounter(MetricName, Value)</summary>
        public void SetCounter(string metricName, double value)
        {
            RequireMetric(metricName);
            lock (_lock)
            {
                _counters[metricName] = value;
            }
        }

        /// <summary>AnalyticsService.IncrementCounter(MetricName, Delta)</summary>
        public void IncrementCounter(string metricName, double delta = 1)
        {
            RequireMetric(metricName);
            lock (_lock)
            {
                _counters[metricName] = _counters.GetValueOrDefault(metricName) + delta;
            }
        }

        /// <summary>AnalyticsService.GetCounter(MetricName)</summary>
        public double GetCounter(string metricName)
        {
            RequireMetric(metricName);
            lock (_lock)
            {
                return _counters.GetValueOrDefault(metricName);
            }
        }

        /// <summary>AnalyticsService.GetEvents(EventName, Limit) — newest last.</summary>
        public IReadOnlyList<AnalyticsEvent> GetEvents(string eventName, int limit = 50)
        {
            if (string.IsNullOrWhiteSpace(eventName))
                throw new ArgumentException("EventName must not be empty.", nameof(eventName));
            MarketplaceService.RequireLimit(limit);
            lock (_lock)
            {
                return _events.Where(e => e.EventName == eventName).TakeLast(limit).ToArray();
            }
        }

        /// <summary>AnalyticsService.ClearEvents(EventName) — clears one event stream.</summary>
        public void ClearEvents(string eventName)
        {
            if (string.IsNullOrWhiteSpace(eventName))
                throw new ArgumentException("EventName must not be empty.", nameof(eventName));
            lock (_lock)
            {
                _events.RemoveAll(e => e.EventName == eventName);
            }
        }

        private void Add(string kind, string eventName, string? username,
            IReadOnlyDictionary<string, object?>? properties)
        {
            if (string.IsNullOrWhiteSpace(eventName))
                throw new ArgumentException($"{kind} message must not be empty.", nameof(eventName));
            lock (_lock)
            {
                _events.Add(new AnalyticsEvent(eventName, username,
                    properties ?? new Dictionary<string, object?>(), _clock()));
            }
        }

        private static void RequireMetric(string metricName)
        {
            if (string.IsNullOrWhiteSpace(metricName))
                throw new ArgumentException("MetricName must not be empty.", nameof(metricName));
        }
    }

    /// <summary>
    /// LocalizationService manages BCP 47 translation tables and resolves
    /// text per player locale (per-player locale defaults to "en").
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#localizationservice
    /// </summary>
    public class LocalizationService
    {
        /// <summary>Fallback player locale, per the docs' "en" example.</summary>
        public const string DefaultLocale = "en";

        private readonly object _lock = new();
        private readonly Dictionary<string, Dictionary<string, string>> _tables =
            new(StringComparer.Ordinal); // key -> locale -> text
        private readonly Dictionary<string, string> _playerLocales = new(StringComparer.Ordinal);

        /// <summary>LocalizationService.Register(Key, Translations)</summary>
        public void Register(string key, IReadOnlyDictionary<string, string> translations)
        {
            RequireKey(key);
            if (translations == null || translations.Count == 0)
                throw new ArgumentException("Translations must not be empty.", nameof(translations));
            foreach (string locale in translations.Keys)
                RequireLocale(locale);

            lock (_lock)
            {
                if (!_tables.TryAdd(key, new Dictionary<string, string>(translations, StringComparer.OrdinalIgnoreCase)))
                    throw new InvalidOperationException($"Key '{key}' is already registered.");
            }
        }

        /// <summary>LocalizationService.Unregister(Key)</summary>
        public void Unregister(string key)
        {
            RequireKey(key);
            lock (_lock)
            {
                if (!_tables.Remove(key))
                    throw new InvalidOperationException($"No translation key '{key}'.");
            }
        }

        /// <summary>LocalizationService.Translate(Key, Locale)</summary>
        public string? Translate(string key, string locale)
        {
            RequireKey(key);
            RequireLocale(locale);
            lock (_lock)
            {
                return _tables.GetValueOrDefault(key)?.GetValueOrDefault(locale);
            }
        }

        /// <summary>LocalizationService.TranslateFor(Key, Username) — uses the player's locale.</summary>
        public string? TranslateFor(string key, string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            return Translate(key, GetLocale(username));
        }

        /// <summary>LocalizationService.GetLocale(Username)</summary>
        public string GetLocale(string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            lock (_lock)
            {
                return _playerLocales.GetValueOrDefault(username, DefaultLocale);
            }
        }

        /// <summary>LocalizationService.SetLocale(Username, Locale)</summary>
        public void SetLocale(string username, string locale)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            RequireLocale(locale);
            lock (_lock)
            {
                _playerLocales[username] = locale;
            }
        }

        /// <summary>LocalizationService.GetSupportedLocales() — every locale seen in registered tables.</summary>
        public IReadOnlyList<string> GetSupportedLocales()
        {
            lock (_lock)
            {
                return _tables.Values.SelectMany(t => t.Keys).Distinct(StringComparer.OrdinalIgnoreCase)
                    .OrderBy(l => l, StringComparer.OrdinalIgnoreCase).ToArray();
            }
        }

        /// <summary>LocalizationService.ImportTable(Translations) — bulk import; existing keys are replaced.</summary>
        public void ImportTable(IReadOnlyDictionary<string, IReadOnlyDictionary<string, string>> translations)
        {
            if (translations == null)
                throw new ArgumentNullException(nameof(translations));
            foreach (var kv in translations)
            {
                if (_tables.ContainsKey(kv.Key))
                    Unregister(kv.Key);
                Register(kv.Key, kv.Value);
            }
        }

        private static void RequireKey(string key)
        {
            if (string.IsNullOrWhiteSpace(key))
                throw new ArgumentException("Key must not be empty.", nameof(key));
        }

        private static void RequireLocale(string locale)
        {
            if (string.IsNullOrWhiteSpace(locale))
                throw new ArgumentException("Locale must not be empty.", nameof(locale));
        }
    }

    /// <summary>
    /// PassService (also known as GamepassService) registers Gamepasses with
    /// optional permission tables, grants/revokes them and checks
    /// permissions granted through any of a player's passes.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#passservice
    /// </summary>
    public class PassService
    {
        /// <summary>A pass definition (GetAll payload).</summary>
        public sealed record PassSpec(string PassId, string Name, string Description,
            IReadOnlyDictionary<string, string> Permissions);

        private readonly object _lock = new();
        private readonly Dictionary<string, PassSpec> _definitions = new(StringComparer.Ordinal);
        private readonly Dictionary<string, HashSet<string>> _granted = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action<string>>> _grantCallbacks = new(StringComparer.Ordinal);

        /// <summary>PassService.Register(PassId, Name, Description, Permissions)</summary>
        public void Register(string passId, string name, string description,
            IReadOnlyDictionary<string, string>? permissions = null)
        {
            RequireId(passId);
            if (string.IsNullOrWhiteSpace(name))
                throw new ArgumentException("Name must not be empty.", nameof(name));
            if (permissions == null)
                permissions = new Dictionary<string, string>();

            lock (_lock)
            {
                if (!_definitions.TryAdd(passId,
                        new PassSpec(passId, name, description,
                            new Dictionary<string, string>(permissions, StringComparer.OrdinalIgnoreCase))))
                    throw new InvalidOperationException($"Pass '{passId}' is already registered.");
            }
        }

        /// <summary>PassService.Unregister(PassId)</summary>
        public void Unregister(string passId)
        {
            RequireId(passId);
            lock (_lock)
            {
                if (!_definitions.Remove(passId))
                    throw new InvalidOperationException($"No pass '{passId}'.");
            }
        }

        /// <summary>PassService.Grant(Username, PassId) — idempotent; OnGrant fires on first grant.</summary>
        public void Grant(string username, string passId)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            RequireDefinition(passId);

            List<Action<string>>? callbacks = null;
            lock (_lock)
            {
                if (!_granted.TryGetValue(username, out var set))
                    _granted[username] = set = new HashSet<string>(StringComparer.Ordinal);
                if (set.Add(passId))
                    _grantCallbacks.TryGetValue(username, out callbacks);
            }
            callbacks?.ForEach(cb => cb(passId));
        }

        /// <summary>PassService.Revoke(Username, PassId)</summary>
        public void Revoke(string username, string passId)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            RequireDefinition(passId);
            lock (_lock)
            {
                _granted.GetValueOrDefault(username)?.Remove(passId);
            }
        }

        /// <summary>PassService.HasPass(Username, PassId)</summary>
        public bool HasPass(string username, string passId)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            RequireDefinition(passId);
            lock (_lock)
            {
                return _granted.GetValueOrDefault(username)?.Contains(passId) == true;
            }
        }

        /// <summary>PassService.GetPasses(Username)</summary>
        public IReadOnlyList<string> GetPasses(string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            lock (_lock)
            {
                return _granted.GetValueOrDefault(username)?.OrderBy(p => p, StringComparer.Ordinal).ToArray()
                    ?? Array.Empty<string>();
            }
        }

        /// <summary>PassService.GetAll()</summary>
        public IReadOnlyList<PassSpec> GetAll()
        {
            lock (_lock)
            {
                return _definitions.Values.ToArray();
            }
        }

        /// <summary>PassService.CheckPermission(Username, Permission) — true when any granted pass carries it.</summary>
        public bool CheckPermission(string username, string permission)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (string.IsNullOrWhiteSpace(permission))
                throw new ArgumentException("Permission must not be empty.", nameof(permission));

            lock (_lock)
            {
                return _granted.GetValueOrDefault(username)?.Any(passId =>
                    _definitions.GetValueOrDefault(passId)?.Permissions.ContainsKey(permission) == true) == true;
            }
        }

        /// <summary>PassService.OnGrant(Username, Callback) — receives the PassId.</summary>
        public void OnGrant(string username, Action<string> callback)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                if (!_grantCallbacks.TryGetValue(username, out var list))
                    _grantCallbacks[username] = list = new List<Action<string>>();
                list.Add(callback);
            }
        }

        private void RequireDefinition(string passId)
        {
            RequireId(passId);
            if (!_definitions.ContainsKey(passId))
                throw new InvalidOperationException($"No pass '{passId}'.");
        }

        private static void RequireId(string passId)
        {
            if (string.IsNullOrWhiteSpace(passId))
                throw new ArgumentException("PassId must not be empty.", nameof(passId));
        }
    }

    /// <summary>
    /// AudioCService controls existing Audio entities: New, Stop, Resume,
    /// Pause, SetVolume, Change and Loop.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#audiocservice
    /// </summary>
    public class AudioCService
    {
        /// <summary>Audio playback states.</summary>
        public enum AudioState
        {
            Stopped,
            Playing,
            Paused,
        }

        /// <summary>One audio entity (tests/tooling).</summary>
        public sealed record AudioSnapshot(string Name, string Id, double Volume, bool Loop, AudioState State);

        private readonly object _lock = new();
        private readonly Dictionary<string, AudioEntity> _entities = new(StringComparer.Ordinal);

        /// <summary>AudioService.New(Name, ID)</summary>
        public void New(string name, string id)
        {
            if (string.IsNullOrWhiteSpace(name))
                throw new ArgumentException("Name must not be empty.", nameof(name));
            if (string.IsNullOrWhiteSpace(id))
                throw new ArgumentException("ID must not be empty.", nameof(id));
            lock (_lock)
            {
                if (!_entities.TryAdd(name, new AudioEntity { Id = id }))
                    throw new InvalidOperationException($"Audio '{name}' already exists.");
            }
        }

        /// <summary>AudioService.Stop(Name)</summary>
        public void Stop(string name) => SetState(name, AudioState.Stopped);

        /// <summary>AudioService.Resume(Name) — starts or continues playback.</summary>
        public void Resume(string name)
        {
            if (string.IsNullOrWhiteSpace(name))
                throw new ArgumentException("Name must not be empty.", nameof(name));
            lock (_lock)
            {
                var entity = RequireEntity(name);
                if (entity.State == AudioState.Playing)
                    throw new InvalidOperationException($"Audio '{name}' is already playing.");
                entity.State = AudioState.Playing;
            }
        }

        /// <summary>AudioService.Pause(Name)</summary>
        public void Pause(string name) => SetState(name, AudioState.Paused, requirePlaying: true);


        /// <summary>AudioService.SetVolume(Name, Volume) — 0.0 to 1.0.</summary>
        public void SetVolume(string name, double volume)
        {
            if (volume is < 0.0 or > 1.0)
                throw new ArgumentException("Volume must be between 0.0 and 1.0.", nameof(volume));
            Mutate(name, entity => entity.Volume = volume);
        }

        /// <summary>AudioService.Change(Name, ID) — swaps the played asset.</summary>
        public void Change(string name, string id)
        {
            if (string.IsNullOrWhiteSpace(id))
                throw new ArgumentException("ID must not be empty.", nameof(id));
            Mutate(name, entity => entity.Id = id);
        }

        /// <summary>AudioService.Loop(Name, Boolean)</summary>
        public void Loop(string name, bool loop) => Mutate(name, entity => entity.Loop = loop);

        /// <summary>Snapshot of one audio entity (tests/tooling).</summary>
        public AudioSnapshot? Get(string name)
        {
            if (string.IsNullOrWhiteSpace(name))
                throw new ArgumentException("Name must not be empty.", nameof(name));
            lock (_lock)
            {
                return _entities.TryGetValue(name, out var e)
                    ? new AudioSnapshot(name, e.Id, e.Volume, e.Loop, e.State)
                    : null;
            }
        }

        private sealed class AudioEntity
        {
            public string Id = "";
            public double Volume = 1.0;
            public bool Loop;
            public AudioState State = AudioState.Stopped;
        }

        private void SetState(string name, AudioState state, bool requirePaused = false, bool requirePlaying = false)
        {
            if (string.IsNullOrWhiteSpace(name))
                throw new ArgumentException("Name must not be empty.", nameof(name));
            lock (_lock)
            {
                var entity = RequireEntity(name);
                if (requirePaused && entity.State != AudioState.Paused)
                    throw new InvalidOperationException($"Audio '{name}' is not paused.");
                if (requirePlaying && entity.State != AudioState.Playing)
                    throw new InvalidOperationException($"Audio '{name}' is not playing.");
                entity.State = state;
            }
        }

        private void Mutate(string name, Action<AudioEntity> mutate)
        {
            if (string.IsNullOrWhiteSpace(name))
                throw new ArgumentException("Name must not be empty.", nameof(name));
            lock (_lock)
            {
                mutate(RequireEntity(name));
            }
        }

        private AudioEntity RequireEntity(string name)
        {
            return _entities.GetValueOrDefault(name)
                ?? throw new InvalidOperationException($"No audio entity '{name}'.");
        }
    }
}
