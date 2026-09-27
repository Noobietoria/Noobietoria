using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Text.Json;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// Request categories mirrored from the Roblox-side DataStore API. All
    /// categories draw from the same server-wide per-minute budget
    /// (15 + 10 x CCU requests per minute) described in the docs.
    /// </summary>
    public enum DataStoreRequestType
    {
        GetAsync = 0,
        SetIncrementAsync = 1,
        UpdateAsync = 2,
        GetSortedAsync = 3,
        SetIncrementSortedAsync = 4,
        OnUpdate = 5,
    }

    /// <summary>
    /// DataStoreService stores game data on our side so it survives server
    /// restarts. Two flavors are supported, per the API docs:
    ///
    /// 1. The raw session API — Init(SessionID), Insert/Get/GetKeys/Delete,
    ///    Close(SessionID). SessionIDs must be base32; a session that sends
    ///    no message for 15 seconds is closed automatically.
    /// 2. The Roblox-side APIs — GetDataStore(name, scope) returning a
    ///    DataStore with GetAsync/SetAsync/UpdateAsync/RemoveAsync, plus
    ///    GetOrderedDataStore for numeric sorting. Here the connection is
    ///    managed automatically, so no explicit Init is needed.
    ///
    /// Limits enforced by this implementation: up to 15 datastores per game,
    /// up to 16 concurrent (explicit) sessions across the whole server, keys
    /// up to 50 ASCII characters, values up to 1 MB of JSON, a total capacity
    /// of 25 MB + 5 MB per visit, and the per-minute request budget.
    ///
    /// v0 keeps data in memory; persistence to the platform database lands
    /// with the backend integration. Clock is injectable so the 15-second
    /// idle timeout and budget windows are testable.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#datastoreservice
    /// </summary>
    public class DataStoreService
    {
        public const int MaxDataStoresPerGame = 15;
        public const int MaxConnections = 16;
        public const int MaxKeyLength = 50;
        public const long MaxValueBytes = 1024 * 1024;
        public const long BaseCapacityBytes = 25L * 1024 * 1024;
        public const long CapacityBytesPerVisit = 5L * 1024 * 1024;
        public const int BaseRequestBudgetPerMinute = 15;
        public const int RequestBudgetPerConcurrentUser = 10;
        public const int IdleTimeoutSeconds = 15;
        public const int RequestWindowMinutes = 1;

        private static readonly string DefaultNamespace = "\u0001";

        private readonly object _lock = new();
        private readonly Func<DateTime> _clock;

        private sealed class Session
        {
            public string Id = "";
            public DateTime LastActivityUtc;
        }

        // sessionId -> session (explicit connections opened with Init).
        private readonly Dictionary<string, Session> _sessions = new(StringComparer.Ordinal);

        // namespace ("name\u0001scope") -> key -> value (JSON text).
        private readonly Dictionary<string, Dictionary<string, string>> _stores = new(StringComparer.Ordinal);

        // Distinct datastore names seen via GetDataStore/GetOrderedDataStore.
        private readonly HashSet<string> _knownStoreNames = new(StringComparer.Ordinal);

        private long _usedBytes;
        private DateTime _windowEndUtc;
        private int _requestsUsedInWindow;

        /// <summary>Concurrent users on this server (N in the budget formula).</summary>
        public int ConcurrentUsers { get; set; }

        /// <summary>Visits the game has served (each adds 5 MB of capacity).</summary>
        public int Visits { get; set; }

        /// <summary>Total bytes this game may store. Override for self-hosted games with different caps.</summary>
        public long TotalCapacityBytes { get; set; } = BaseCapacityBytes;

        public DataStoreService(Func<DateTime>? clock = null)
        {
            _clock = clock ?? DefaultClock;
            _windowEndUtc = _clock() + TimeSpan.FromMinutes(RequestWindowMinutes);
        }

        private static DateTime DefaultClock() => DateTime.UtcNow;

        // ---- session API ---------------------------------------------------

        /// <summary>
        /// DataStoreService.Init(SessionID) — opens a connection. SessionID
        /// is a random string you generate; anything base32 works.
        /// </summary>
        public void Init(string sessionId)
        {
            lock (_lock)
            {
                RequireValidBase32(sessionId, nameof(sessionId));
                SweepExpiredSessions();

                if (_sessions.ContainsKey(sessionId))
                    throw new InvalidOperationException($"DataStore session '{sessionId}' is already open.");
                if (_sessions.Count >= MaxConnections)
                    throw new InvalidOperationException(
                        $"Too many concurrent DataStore connections ({MaxConnections} maximum across the whole server). Close one first.");

                _sessions[sessionId] = new Session { Id = sessionId, LastActivityUtc = _clock() };
            }
        }

        /// <summary>
        /// DataStoreService.Close(SessionID) — closes the connection when done.
        /// </summary>
        public void Close(string sessionId)
        {
            lock (_lock)
            {
                RequireSession(sessionId);
                _sessions.Remove(sessionId);
            }
        }

        public bool IsSessionOpen(string sessionId)
        {
            lock (_lock)
            {
                SweepExpiredSessions();
                return sessionId != null && _sessions.ContainsKey(sessionId);
            }
        }

        /// <summary>
        /// DataStoreService.Insert(SessionID, Key, Value) — creates or
        /// replaces Key with a JSON Value.
        /// </summary>
        public void Insert(string sessionId, string key, string value)
        {
            lock (_lock)
            {
                var session = RequireSession(sessionId);
                ConsumeRequestBudget();
                ValidateKey(key);
                ValidateValue(value);
                Upsert(DefaultNamespace, key, value);
                session.LastActivityUtc = _clock();
            }
        }

        /// <summary>
        /// DataStoreService.Get(SessionID, Key) — the stored JSON value, or
        /// null when the key does not exist.
        /// </summary>
        public string? Get(string sessionId, string key)
        {
            lock (_lock)
            {
                var session = RequireSession(sessionId);
                ConsumeRequestBudget();
                ValidateKey(key);
                session.LastActivityUtc = _clock();
                return GetRaw(DefaultNamespace, key);
            }
        }

        /// <summary>
        /// DataStoreService.GetKeys(SessionID) — every key in the store.
        /// </summary>
        public IReadOnlyList<string> GetKeys(string sessionId)
        {
            lock (_lock)
            {
                var session = RequireSession(sessionId);
                ConsumeRequestBudget();
                session.LastActivityUtc = _clock();
                return GetStore(DefaultNamespace).Keys.ToArray();
            }
        }

        /// <summary>
        /// DataStoreService.Delete(SessionID, Key) — true when the key
        /// existed and was removed.
        /// </summary>
        public bool Delete(string sessionId, string key)
        {
            lock (_lock)
            {
                var session = RequireSession(sessionId);
                ConsumeRequestBudget();
                ValidateKey(key);
                session.LastActivityUtc = _clock();
                return DeleteRaw(DefaultNamespace, key);
            }
        }

        /// <summary>
        /// Removes sessions that sent no message for 15 seconds. The host
        /// (server) calls this periodically; operations also sweep lazily.
        /// </summary>
        public void SweepExpiredSessions()
        {
            lock (_lock)
            {
                DateTime now = _clock();
                var expired = _sessions
                    .Where(kv => (now - kv.Value.LastActivityUtc).TotalSeconds >= IdleTimeoutSeconds)
                    .Select(kv => kv.Key)
                    .ToArray();
                foreach (string id in expired)
                    _sessions.Remove(id);
            }
        }

        // ---- Roblox-side APIs ----------------------------------------------

        /// <summary>
        /// DataStoreService:GetDataStore(Name, Scope) — up to 15 distinct
        /// datastores per game. The connection is managed automatically.
        /// </summary>
        public DataStore GetDataStore(string name, string scope = "global")
        {
            lock (_lock)
            {
                if (string.IsNullOrWhiteSpace(name))
                    throw new ArgumentException("DataStore name must not be empty.", nameof(name));

                if (_knownStoreNames.Add(name) && _knownStoreNames.Count > MaxDataStoresPerGame)
                {
                    _knownStoreNames.Remove(name);
                    throw new InvalidOperationException(
                        $"Too many datastores ({MaxDataStoresPerGame} per game maximum).");
                }

                return new DataStore(this, name, scope);
            }
        }

        /// <summary>
        /// DataStoreService:GetOrderedDataStore(Name, Scope) — a DataStore
        /// whose numeric values can be fetched in sorted order.
        /// </summary>
        public OrderedDataStore GetOrderedDataStore(string name, string scope = "global")
        {
            lock (_lock)
            {
                // Reuse the name registration by going through GetDataStore's
                // validation, but hand back the ordered flavor.
                var store = GetDataStore(name, scope);
                return new OrderedDataStore(this, store.Name, store.Scope);
            }
        }

        /// <summary>
        /// DataStoreService:GetRequestBudgetForRequestType(RequestType) —
        /// requests still available in the current one-minute window. The
        /// docs define one server-wide budget (15 + 10 x CCU per minute),
        /// so every request type draws from the same pool.
        /// </summary>
        public int GetRequestBudgetForRequestType(DataStoreRequestType requestType)
        {
            lock (_lock)
            {
                RollRequestWindow();
                return RequestBudget() - _requestsUsedInWindow;
            }
        }

        // ---- internals (shared by the session API and DataStore views) -----

        internal string? GetRawCompat(string @namespace, string key)
        {
            lock (_lock)
            {
                ConsumeRequestBudget();
                ValidateKey(key);
                return GetRaw(@namespace, key);
            }
        }

        internal void SetRawCompat(string @namespace, string key, string value)
        {
            lock (_lock)
            {
                ConsumeRequestBudget();
                ValidateKey(key);
                ValidateValue(value);
                Upsert(@namespace, key, value);
            }
        }

        internal bool DeleteRawCompat(string @namespace, string key)
        {
            lock (_lock)
            {
                ConsumeRequestBudget();
                ValidateKey(key);
                return DeleteRaw(@namespace, key);
            }
        }

        /// <summary>
        /// One request for the whole UpdateAsync: current is read, the
        /// transform result is stored, and a null result deletes the key
        /// (matching the Roblox contract).
        /// </summary>
        internal void UpdateRawCompat(string @namespace, string key, Func<string?, string?> transform)
        {
            lock (_lock)
            {
                ConsumeRequestBudget();
                ValidateKey(key);
                if (transform == null)
                    throw new ArgumentNullException(nameof(transform));

                string? current = GetRaw(@namespace, key);
                string? updated = transform(current);

                if (updated == null)
                {
                    DeleteRaw(@namespace, key);
                    return;
                }

                ValidateValue(updated);
                Upsert(@namespace, key, updated);
            }
        }

        internal IReadOnlyList<KeyValuePair<string, double>> GetSortedRawCompat(
            string @namespace, bool ascending, int limit, double? minValue, double? maxValue)
        {
            lock (_lock)
            {
                ConsumeRequestBudget();
                if (limit < 1)
                    throw new ArgumentOutOfRangeException(nameof(limit), "Limit must be >= 1.");

                var numeric = new List<KeyValuePair<string, double>>();
                foreach (var kv in GetStore(@namespace))
                {
                    if (!TryParseNumber(kv.Value, out double number))
                        continue;
                    if (minValue.HasValue && number < minValue.Value)
                        continue;
                    if (maxValue.HasValue && number > maxValue.Value)
                        continue;
                    numeric.Add(new KeyValuePair<string, double>(kv.Key, number));
                }

                return (ascending ? numeric.OrderBy(kv => kv.Value) : numeric.OrderByDescending(kv => kv.Value))
                    .Take(limit)
                    .ToArray();
            }
        }

        private Dictionary<string, string> GetStore(string @namespace)
        {
            if (!_stores.TryGetValue(@namespace, out var store))
            {
                store = new Dictionary<string, string>(StringComparer.Ordinal);
                _stores[@namespace] = store;
            }
            return store;
        }

        private string? GetRaw(string @namespace, string key) =>
            GetStore(@namespace).TryGetValue(key, out var value) ? value : null;

        private void Upsert(string @namespace, string key, string value)
        {
            var store = GetStore(@namespace);
            long newBytes = Encoding.UTF8.GetByteCount(value);
            long oldBytes = store.TryGetValue(key, out var old) ? Encoding.UTF8.GetByteCount(old) : 0;

            if (_usedBytes - oldBytes + newBytes > TotalCapacityBytes)
                throw new InvalidOperationException(
                    $"DataStore capacity exceeded ({TotalCapacityBytes} bytes; {_usedBytes} in use). Increase Visits or free some data.");

            store[key] = value;
            _usedBytes = _usedBytes - oldBytes + newBytes;
        }

        private bool DeleteRaw(string @namespace, string key)
        {
            var store = GetStore(@namespace);
            if (!store.TryGetValue(key, out var old))
                return false;

            _usedBytes -= Encoding.UTF8.GetByteCount(old);
            store.Remove(key);
            return true;
        }

        private Session RequireSession(string sessionId)
        {
            RequireValidBase32(sessionId, nameof(sessionId));
            SweepExpiredSessions();

            if (!_sessions.TryGetValue(sessionId, out var session))
                throw new InvalidOperationException(
                    $"No open DataStore session '{sessionId}' — it was closed or timed out after {IdleTimeoutSeconds}s of inactivity. Call Init again.");

            return session;
        }

        private static void RequireValidBase32(string? sessionId, string paramName)
        {
            if (string.IsNullOrWhiteSpace(sessionId) || !IsValidBase32(sessionId))
                throw new ArgumentException(
                    "SessionID must be a non-empty base32 string (letters A-Z, digits 2-7).", paramName);
        }

        private static bool IsValidBase32(string sessionId)
        {
            int end = sessionId.Length;
            while (end > 0 && sessionId[end - 1] == '=')
                end--;
            if (end == 0)
                return false;

            for (int i = 0; i < end; i++)
            {
                char c = sessionId[i];
                bool valid = (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') || (c >= '2' && c <= '7');
                if (!valid)
                    return false;
            }
            return true;
        }

        private static void ValidateKey(string key)
        {
            if (string.IsNullOrEmpty(key))
                throw new ArgumentException("Key must not be empty.", nameof(key));
            if (key.Length > MaxKeyLength)
                throw new ArgumentException($"Key must be at most {MaxKeyLength} ASCII characters.", nameof(key));
            foreach (char c in key)
            {
                if (c > 127)
                    throw new ArgumentException("Key must contain ASCII characters only.", nameof(key));
            }
        }

        private static void ValidateValue(string value)
        {
            if (value == null)
                throw new ArgumentNullException(nameof(value));
            long bytes = Encoding.UTF8.GetByteCount(value);
            if (bytes > MaxValueBytes)
                throw new ArgumentException($"Value must be at most {MaxValueBytes} bytes (1 MB).", nameof(value));

            try
            {
                using var _ = JsonDocument.Parse(value);
            }
            catch (JsonException e)
            {
                throw new ArgumentException("Value must be valid JSON.", nameof(value), e);
            }
        }

        private static bool TryParseNumber(string json, out double number)
        {
            number = 0;
            try
            {
                using var document = JsonDocument.Parse(json);
                if (document.RootElement.ValueKind != JsonValueKind.Number)
                    return false;
                return document.RootElement.TryGetDouble(out number);
            }
            catch (JsonException)
            {
                return false;
            }
        }

        private int RequestBudget() =>
            BaseRequestBudgetPerMinute + RequestBudgetPerConcurrentUser * Math.Max(0, ConcurrentUsers);

        private void ConsumeRequestBudget()
        {
            RollRequestWindow();
            if (_requestsUsedInWindow >= RequestBudget())
                throw new InvalidOperationException(
                    $"DataStore request budget exceeded ({RequestBudget()} requests per minute). Wait for the window to reset.");

            _requestsUsedInWindow++;
        }

        private void RollRequestWindow()
        {
            DateTime now = _clock();
            if (now >= _windowEndUtc)
            {
                _windowEndUtc = now + TimeSpan.FromMinutes(RequestWindowMinutes);
                _requestsUsedInWindow = 0;
            }
        }
    }

    /// <summary>
    /// A named view over the store, as handed out by
    /// DataStoreService:GetDataStore(Name, Scope). Operations consume the
    /// server-wide request budget; the connection is managed automatically.
    /// </summary>
    public class DataStore
    {
        private readonly DataStoreService _service;
        private readonly string _namespace;

        protected internal DataStore(DataStoreService service, string name, string scope)
        {
            _service = service;
            _namespace = name + "\u0001" + scope;
            Name = name;
            Scope = scope;
        }

        public string Name { get; }
        public string Scope { get; }

        internal DataStoreService Service => _service;
        internal string Namespace => _namespace;

        /// <summary>DataStore:GetAsync(Key) — the stored JSON value or null.</summary>
        public string? GetAsync(string key) => _service.GetRawCompat(_namespace, key);

        /// <summary>
        /// DataStore:SetAsync(Key, Value, UserIds, Options) — creates or
        /// replaces Key with a JSON Value. UserIds/Options are accepted for
        /// signature parity and currently unused.
        /// </summary>
        public void SetAsync(
            string key,
            string value,
            IReadOnlyList<int>? userIds = null,
            IReadOnlyDictionary<string, object?>? options = null)
            => _service.SetRawCompat(_namespace, key, value);

        /// <summary>
        /// DataStore:UpdateAsync(Key, Transform) — transforms the current
        /// value (null when absent); returning null deletes the key.
        /// </summary>
        public void UpdateAsync(string key, Func<string?, string?> transform)
            => _service.UpdateRawCompat(_namespace, key, transform);

        /// <summary>DataStore:RemoveAsync(Key) — true when the key existed.</summary>
        public bool RemoveAsync(string key) => _service.DeleteRawCompat(_namespace, key);
    }

    /// <summary>
    /// The flavor handed out by DataStoreService:GetOrderedDataStore —
    /// numeric values can be fetched in sorted order via GetSortedAsync.
    /// </summary>
    public sealed class OrderedDataStore : DataStore
    {
        internal OrderedDataStore(DataStoreService service, string name, string scope)
            : base(service, name, scope)
        {
        }

        /// <summary>
        /// OrderedDataStore:GetSortedAsync(Ascending, Limit, MinValue, MaxValue)
        /// — keys with numeric JSON values, sorted by value. Non-numeric
        /// values are skipped.
        /// </summary>
        public IReadOnlyList<KeyValuePair<string, double>> GetSortedAsync(
            bool ascending, int limit, double? minValue = null, double? maxValue = null)
            => Service.GetSortedRawCompat(Namespace, ascending, limit, minValue, maxValue);
    }
}
