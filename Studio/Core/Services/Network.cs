using System;
using System.Collections.Generic;
using System.Linq;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// Validation helpers for values crossing the Luau boundary. The docs
    /// require network payloads to be serializable: string, number, boolean
    /// or plain data tables of those.
    /// </summary>
    public static class LuauValues
    {
        /// <summary>True when the value can cross the Luau boundary unchanged.</summary>
        public static bool IsSerializable(object? value) => value switch
        {
            null => true,
            string => true,
            bool => true,
            byte or sbyte or short or ushort or int or uint or long or ulong or float or double or decimal => true,
            IReadOnlyDictionary<string, object?> table => table.Values.All(IsSerializable),
            IDictionary<string, object?> table => table.Values.All(IsSerializable),
            IEnumerable<object?> list => list.All(IsSerializable),
            _ => false,
        };

        /// <summary>Throws when a value is not a serializable Luau value.</summary>
        public static void EnsureSerializable(object? value, string paramName = "value")
        {
            if (!IsSerializable(value))
                throw new ArgumentException(
                    "Value must be a serializable Luau value (string, number, boolean, table or list of those).",
                    paramName);
        }
    }

    /// <summary>
    /// Shared callback state for NetworkService instances. Attach the same
    /// hub to the server-side and client-side services so they can see each
    /// other's handlers (a hub is a single in-process message bus).
    /// </summary>
    public sealed class NetworkHub
    {
        public readonly Dictionary<string, List<Action<string?, object?[]>>> ServerCallbacks =
            new(StringComparer.Ordinal);

        public readonly Dictionary<string, List<Action<object?[]>>> ClientCallbacks =
            new(StringComparer.Ordinal);
    }

    /// <summary>
    /// NetworkService controls the NetworkEvent Instance for two-way
    /// client/server messaging. FireServer/OnClientEvent are client-only,
    /// FireClient/FireAllClients/OnServerEvent are server-only — calling
    /// from the wrong side raises an error (modelled with the Side option).
    /// Payloads must be serializable Luau values.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#networkservice
    /// </summary>
    public class NetworkService
    {
        /// <summary>Which side this service instance acts on.</summary>
        public enum RpcSide
        {
            Both,
            Server,
            Client,
        }

        private readonly InstanceService? _instances;
        private readonly RpcSide _side;
        private readonly NetworkHub _hub;

        public NetworkService(InstanceService? instances = null, RpcSide side = RpcSide.Both, NetworkHub? hub = null)
        {
            _instances = instances;
            _side = side;
            _hub = hub ?? new NetworkHub();
        }

        /// <summary>NetworkService.Create(Name, Parent) — creates the NetworkEvent Instance.</summary>
        public void Create(string name, string parent = "ReplicatedStorage", int index = 1)
        {
            RequireEventName(name);
            if (_instances == null)
                throw new InvalidOperationException(
                    "NetworkService.Create requires an InstanceService to host the NetworkEvent Instance.");

            string uniqueName = index > 1 ? $"{name}#{index}" : name;
            _instances.Create(uniqueName, "NetworkEvent", parent);
        }

        /// <summary>NetworkService.Destroy(Name, Index)</summary>
        public void Destroy(string name, int index = 1)
        {
            RequireEventName(name);
            lock (_hub)
            {
                _hub.ServerCallbacks.Remove(Key(name, index));
                _hub.ClientCallbacks.Remove(Key(name, index));
            }
            _instances?.Destroy(index > 1 ? $"{name}#{index}" : name);
        }

        /// <summary>NetworkService.FireClient(Name, Username, ..., Index) — server only.</summary>
        public void FireClient(string name, string username, params object?[] args)
        {
            RequireSide(nameof(FireClient), serverOnly: true);
            RequireEventName(name);
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty (use FireAllClients for everyone).", nameof(username));
            LuauValues.EnsureSerializable(args, "args");

            var callbacks = Snapshot(_hub.ClientCallbacks, Key(name, 1));
            callbacks?.ForEach(cb => cb(args));
        }

        /// <summary>NetworkService.FireAllClients(Name, ..., Index)</summary>
        public void FireAllClients(string name, params object?[] args)
        {
            RequireSide(nameof(FireAllClients), serverOnly: true);
            RequireEventName(name);
            LuauValues.EnsureSerializable(args, "args");

            var callbacks = Snapshot(_hub.ClientCallbacks, Key(name, 1));
            callbacks?.ForEach(cb => cb(args));
        }

        /// <summary>NetworkService.OnServerEvent(Name, Callback, Index) — server only.</summary>
        public void OnServerEvent(string name, Action<string?, object?[]> callback, int index = 1)
        {
            RequireSide(nameof(OnServerEvent), serverOnly: true);
            RequireEventName(name);
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_hub)
            {
                if (!_hub.ServerCallbacks.TryGetValue(Key(name, index), out var list))
                    _hub.ServerCallbacks[Key(name, index)] = list = new List<Action<string?, object?[]>>();
                list.Add(callback);
            }
        }

        /// <summary>NetworkService.FireServer(Name, ..., Index) — client only.</summary>
        public void FireServer(string name, params object?[] args)
        {
            RequireSide(nameof(FireServer), serverOnly: false);
            RequireEventName(name);
            LuauValues.EnsureSerializable(args, "args");

            List<Action<string?, object?[]>>? handlers;
            lock (_hub)
            {
                handlers = _hub.ServerCallbacks.TryGetValue(Key(name, 1), out var list) ? list.ToList() : null;
            }
            handlers?.ForEach(cb => cb(null, args));
        }

        /// <summary>NetworkService.OnClientEvent(Name, Callback, Index) — client only.</summary>
        public void OnClientEvent(string name, Action<object?[]> callback, int index = 1)
        {
            RequireSide(nameof(OnClientEvent), serverOnly: false);
            RequireEventName(name);
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_hub)
            {
                if (!_hub.ClientCallbacks.TryGetValue(Key(name, index), out var list))
                    _hub.ClientCallbacks[Key(name, index)] = list = new List<Action<object?[]>>();
                list.Add(callback);
            }
        }

        /// <summary>NetworkService.Disconnect(Name, Index) — drops registered callbacks on both sides.</summary>
        public void Disconnect(string name, int index = 1)
        {
            RequireEventName(name);
            lock (_hub)
            {
                _hub.ServerCallbacks.Remove(Key(name, index));
                _hub.ClientCallbacks.Remove(Key(name, index));
            }
        }

        private void RequireSide(string methodName, bool serverOnly)
        {
            if (_side == RpcSide.Both)
                return;
            if (serverOnly && _side != RpcSide.Server)
                throw new InvalidOperationException($"{methodName} can only be called from a ServerScript.");
            if (!serverOnly && _side != RpcSide.Client)
                throw new InvalidOperationException($"{methodName} can only be called from a ClientScript.");
        }

        private static List<Action<object?[]>>? Snapshot(
            Dictionary<string, List<Action<object?[]>>> source, string key)
        {
            lock (source)
            {
                return source.TryGetValue(key, out var list) ? list.ToList() : null;
            }
        }

        private static List<Action<string?, object?[]>>? Snapshot(
            Dictionary<string, List<Action<string?, object?[]>>> source, string key)
        {
            lock (source)
            {
                return source.TryGetValue(key, out var list) ? list.ToList() : null;
            }
        }

        private static string Key(string name, int index) => name + "\u0001" + index;

        private static void RequireEventName(string name)
        {
            if (string.IsNullOrWhiteSpace(name))
                throw new ArgumentException("NetworkEvent name must not be empty.", nameof(name));
        }
    }

    /// <summary>
    /// Shared handler state for NetworkEventService instances (server-side
    /// and client-side services attached to the same hub communicate).
    /// </summary>
    public sealed class NetworkEventHub
    {
        public readonly Dictionary<string, List<Action<string?, object?[]>>> ServerCallbacks =
            new(StringComparer.Ordinal);

        public readonly Dictionary<string, List<(string? Owner, Action<object?[]> Callback)>> ClientCallbacks =
            new(StringComparer.Ordinal);

        public readonly Dictionary<string, Func<object?[], object?>> ServerInvokers = new(StringComparer.Ordinal);

        public readonly HashSet<string> Declared = new(StringComparer.Ordinal);
    }

    /// <summary>
    /// NetworkEventService is the Luau-managed alternative to the NetworkEvent
    /// Instance: same messaging model plus InvokeServer/OnServerInvoke
    /// (request/response) and FireAllClientsExcept. Requires the caller's
    /// side so the documented server-only/client-only rules can be enforced.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#networkeventservice
    /// </summary>
    public class NetworkEventService
    {
        private readonly NetworkService.RpcSide _side;
        private readonly NetworkEventHub _hub;

        /// <param name="side">The calling script's side; required for the side rules.</param>
        /// <param name="hub">Shared state; attach the same hub to the server and client instances.</param>
        public NetworkEventService(NetworkService.RpcSide side = NetworkService.RpcSide.Both, NetworkEventHub? hub = null)
        {
            _side = side;
            _hub = hub ?? new NetworkEventHub();
        }

        /// <summary>NetworkEventService.OnServerEvent(Name, Callback) — server only.</summary>
        public void OnServerEvent(string name, Action<string?, object?[]> callback)
        {
            RequireSide(nameof(OnServerEvent), serverOnly: true);
            RequireName(name);
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_hub)
            {
                if (!_hub.ServerCallbacks.TryGetValue(name, out var list))
                    _hub.ServerCallbacks[name] = list = new List<Action<string?, object?[]>>();
                list.Add(callback);
            }
        }

        /// <summary>NetworkEventService.FireClient(Name, Username, ...)</summary>
        public void FireClient(string name, string username, params object?[] args)
        {
            RequireSide(nameof(FireClient), serverOnly: true);
            LuauValues.EnsureSerializable(args, "args");
            FireToClients(name, excluded: username, args);
        }

        /// <summary>NetworkEventService.FireAllClients(Name, ...)</summary>
        public void FireAllClients(string name, params object?[] args)
        {
            RequireSide(nameof(FireAllClients), serverOnly: true);
            LuauValues.EnsureSerializable(args, "args");
            FireToClients(name, excluded: null, args);
        }

        /// <summary>NetworkEventService.FireAllClientsExcept(Name, ExcludedUsername, ...)</summary>
        public void FireAllClientsExcept(string name, string excludedUsername, params object?[] args)
        {
            RequireSide(nameof(FireAllClientsExcept), serverOnly: true);
            LuauValues.EnsureSerializable(args, "args");
            FireToClients(name, excluded: excludedUsername, args);
        }

        /// <summary>NetworkEventService.FireServer(Name, ...) — client only.</summary>
        public void FireServer(string name, params object?[] args)
        {
            RequireSide(nameof(FireServer), serverOnly: false);
            LuauValues.EnsureSerializable(args, "args");
            List<Action<string?, object?[]>>? handlers;
            lock (_hub)
            {
                handlers = _hub.ServerCallbacks.TryGetValue(name, out var list) ? list.ToList() : null;
            }
            handlers?.ForEach(cb => cb(null, args));
        }

        /// <summary>NetworkEventService.OnClientEvent(Name, Callback) — client only. Owner is the player the client script belongs to.</summary>
        public void OnClientEvent(string name, Action<object?[]> callback, string? owner = null)
        {
            RequireSide(nameof(OnClientEvent), serverOnly: false);
            RequireName(name);
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_hub)
            {
                if (!_hub.ClientCallbacks.TryGetValue(name, out var list))
                    _hub.ClientCallbacks[name] = list = new List<(string? Owner, Action<object?[]> Callback)>();
                list.Add((owner, callback));
            }
        }

        /// <summary>NetworkEventService.InvokeServer(Name, ...) — client only; returns the OnServerInvoke reply.</summary>
        public object? InvokeServer(string name, params object?[] args)
        {
            RequireSide(nameof(InvokeServer), serverOnly: false);
            LuauValues.EnsureSerializable(args, "args");
            Func<object?[], object?>? invoker;
            lock (_hub)
            {
                _hub.ServerInvokers.TryGetValue(name, out invoker);
            }
            if (invoker == null)
                throw new InvalidOperationException($"No OnServerInvoke handler registered for '{name}'.");
            return invoker(args);
        }

        /// <summary>NetworkEventService.OnServerInvoke(Name, Callback) — server only; the handler returns the reply.</summary>
        public void OnServerInvoke(string name, Func<object?[], object?> callback)
        {
            RequireSide(nameof(OnServerInvoke), serverOnly: true);
            RequireName(name);
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_hub)
            {
                _hub.ServerInvokers[name] = callback;
            }
        }

        /// <summary>NetworkEventService.Disconnect(Name) — drops all handlers for the event.</summary>
        public void Disconnect(string name)
        {
            RequireName(name);
            lock (_hub)
            {
                _hub.ServerCallbacks.Remove(name);
                _hub.ClientCallbacks.Remove(name);
                _hub.ServerInvokers.Remove(name);
            }
        }

        /// <summary>
        /// NetworkEventService.Exists(Name) — true when a NetworkEvent with
        /// that name is declared. Declarations are registered through
        /// <see cref="Declare"/> by the Luau binding.
        /// </summary>
        public bool Exists(string name)
        {
            RequireName(name);
            lock (_hub)
            {
                return _hub.Declared.Contains(name);
            }
        }

        /// <summary>Declares an event (the Luau binding calls this when a script declares one).</summary>
        public void Declare(string name)
        {
            RequireName(name);
            lock (_hub)
            {
                _hub.Declared.Add(name);
            }
        }

        private void FireToClients(string name, string? excluded, object?[] args)
        {
            RequireName(name);
            List<(string? Owner, Action<object?[]> Callback)>? handlers;
            lock (_hub)
            {
                handlers = _hub.ClientCallbacks.TryGetValue(name, out var list) ? list.ToList() : null;
            }
            if (handlers == null)
                return;
            foreach (var (owner, callback) in handlers)
            {
                if (excluded != null && string.Equals(owner, excluded, StringComparison.Ordinal))
                    continue;
                callback(args);
            }
        }

        private static void RequireName(string name)
        {
            if (string.IsNullOrWhiteSpace(name))
                throw new ArgumentException("Name must not be empty.", nameof(name));
        }

        private void RequireSide(string methodName, bool serverOnly)
        {
            if (_side == NetworkService.RpcSide.Both)
                return;
            if (serverOnly && _side != NetworkService.RpcSide.Server)
                throw new InvalidOperationException($"{methodName} can only be called from a ServerScript.");
            if (!serverOnly && _side != NetworkService.RpcSide.Client)
                throw new InvalidOperationException($"{methodName} can only be called from a ClientScript.");
        }
    }
}
