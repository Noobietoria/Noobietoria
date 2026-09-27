using System;
using System.Collections.Generic;
using System.Linq;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// Coordinate table used for Instances that live in 3D space (Part, Model, ...).
    /// Mirrors the {X, Y, Z} table shape described in the public API docs.
    /// </summary>
    public readonly struct Vector3Data
    {
        public double X { get; }
        public double Y { get; }
        public double Z { get; }

        public Vector3Data(double x, double y, double z)
        {
            X = x;
            Y = y;
            Z = z;
        }
    }

    /// <summary>
    /// InstanceService lets you create, delete, look up GUIDs for, and move
    /// Instances (Part, Model, NetworkEvent, ServerScript, ClientScript,
    /// ModuleScript, ...) within Workspace or other containers
    /// (ReplicatedStorage, ServerStorage, StarterPlayerScripts, ...).
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#instanceservice
    /// </summary>
    public class InstanceService
    {
        // Root containers known to the Studio out of the box. Additional
        // containers can be registered via RegisterRootContainer for custom
        // places / plugins.
        private static readonly string[] DefaultRootContainers =
        {
            "Workspace",
            "ReplicatedStorage",
            "ServerStorage",
            "StarterPlayerScripts",
        };

        private readonly HashSet<string> _rootContainers = new(DefaultRootContainers, StringComparer.OrdinalIgnoreCase);

        // Instances indexed by Name, in creation order, so that Index (1-based)
        // resolves deterministically for same-named Instances.
        private readonly Dictionary<string, List<Instance>> _byName = new(StringComparer.Ordinal);

        // Instances indexed by GUID for O(1) GUID lookups.
        private readonly Dictionary<string, Instance> _byGuid = new(StringComparer.Ordinal);

        /// <summary>
        /// Instance types that are expected to carry Luau source code
        /// (ServerScript, ClientScript, ModuleScript). Kept as a set so
        /// Create() can validate the Properties.Source contract described
        /// in the docs.
        /// </summary>
        private static readonly HashSet<string> ScriptTypes = new(StringComparer.OrdinalIgnoreCase)
        {
            "ServerScript",
            "ClientScript",
            "ModuleScript",
        };

        public void RegisterRootContainer(string name) => _rootContainers.Add(name);

        /// <summary>
        /// InstanceService.Create(Name, Type, Parent, Properties)
        /// </summary>
        public Instance Create(string name, string type, string parent, IDictionary<string, object?>? properties = null)
        {
            if (string.IsNullOrWhiteSpace(name))
                throw new ArgumentException("Name must not be empty.", nameof(name));
            if (string.IsNullOrWhiteSpace(type))
                throw new ArgumentException("Type must not be empty.", nameof(type));
            if (string.IsNullOrWhiteSpace(parent))
                throw new ArgumentException("Parent must not be empty.", nameof(parent));

            var parentInstance = ResolveContainerOrInstance(parent);
            string rootContainer = parentInstance is Instance pi ? pi.RootContainer : parent;

            var instance = new Instance(name, type, rootContainer);

            if (properties != null)
            {
                foreach (var kv in properties)
                    instance.SetProperty(kv.Key, kv.Value);
            }

            // ServerScript / ClientScript default to Enabled = true unless
            // explicitly overridden, per the documented contract.
            if (ScriptTypes.Contains(type) && !instance.Properties.ContainsKey("Enabled"))
                instance.SetProperty("Enabled", true);

            if (string.Equals(type, "ModuleScript", StringComparison.OrdinalIgnoreCase)
                && instance.GetProperty<string>("Source") is null)
            {
                throw new InvalidOperationException(
                    "ModuleScript requires Properties.Source to contain code that returns a value.");
            }

            if (parentInstance is Instance parentAsInstance)
                instance.SetParent(parentAsInstance);

            Register(instance);
            return instance;
        }

        /// <summary>
        /// InstanceService.Destroy(Name, Index)
        /// </summary>
        public void Destroy(string name, int index = 1)
        {
            var instance = Find(name, index);
            if (instance == null)
                return;

            DestroyInstance(instance);
        }

        /// <summary>
        /// InstanceService.DestroyByGUID(GUID)
        /// </summary>
        public void DestroyByGUID(string guid)
        {
            if (_byGuid.TryGetValue(guid, out var instance))
                DestroyInstance(instance);
        }

        /// <summary>
        /// InstanceService.GetGUID(Name, Index) — Index defaults to 1.
        /// </summary>
        public string? GetGUID(string name, int index = 1) => Find(name, index)?.Guid;

        /// <summary>
        /// InstanceService.GetGUIDs(Name) — GUIDs of every same-named Instance,
        /// ordered by creation time.
        /// </summary>
        public IReadOnlyList<string> GetGUIDs(string name)
        {
            if (!_byName.TryGetValue(name, out var list))
                return Array.Empty<string>();
            return list.Select(i => i.Guid).ToArray();
        }

        /// <summary>
        /// InstanceService.GetByGUID(GUID)
        /// </summary>
        public Instance? GetByGUID(string guid) =>
            _byGuid.TryGetValue(guid, out var instance) ? instance : null;

        /// <summary>
        /// InstanceService.Move(Name, Position, Index)
        /// </summary>
        public void Move(string name, Vector3Data position, int index = 1)
        {
            var instance = Find(name, index);
            if (instance == null)
                throw new InvalidOperationException($"No Instance named '{name}' at Index {index}.");
            MoveInstance(instance, position);
        }

        /// <summary>
        /// InstanceService.MoveByGUID(GUID, Position)
        /// </summary>
        public void MoveByGUID(string guid, Vector3Data position)
        {
            if (!_byGuid.TryGetValue(guid, out var instance))
                throw new InvalidOperationException($"No Instance with GUID '{guid}'.");
            MoveInstance(instance, position);
        }

        /// <summary>
        /// InstanceService.SetEnabled(Name, Boolean, Index)
        /// Applies to ServerScript / ClientScript.
        /// </summary>
        public void SetEnabled(string name, bool enabled, int index = 1)
        {
            var instance = Find(name, index);
            if (instance == null)
                throw new InvalidOperationException($"No Instance named '{name}' at Index {index}.");
            instance.SetProperty("Enabled", enabled);
        }

        /// <summary>
        /// InstanceService.IsEnabled(Name, Index)
        /// </summary>
        public bool IsEnabled(string name, int index = 1)
        {
            var instance = Find(name, index);
            return instance?.GetProperty("Enabled", true) ?? false;
        }

        /// <summary>
        /// InstanceService.require(Name, Index) — loads and caches the value
        /// returned by a ModuleScript's Source. Actual Luau execution is
        /// delegated to ILuauRuntime so this service stays engine-agnostic;
        /// see Studio/Core/ILuauRuntime.cs.
        /// </summary>
        public object? Require(ILuauRuntime runtime, string name, int index = 1)
        {
            var instance = Find(name, index);
            return RequireInstance(runtime, instance, name);
        }

        /// <summary>
        /// InstanceService.requireByGUID(GUID)
        /// </summary>
        public object? RequireByGUID(ILuauRuntime runtime, string guid)
        {
            _byGuid.TryGetValue(guid, out var instance);
            return RequireInstance(runtime, instance, guid);
        }

        // ---- internals ----------------------------------------------------

        private object? RequireInstance(ILuauRuntime runtime, Instance? instance, string reference)
        {
            if (instance == null)
                throw new InvalidOperationException($"No Instance found for '{reference}'.");
            if (!string.Equals(instance.Type, "ModuleScript", StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException($"'{reference}' is not a ModuleScript.");

            return runtime.RequireModule(instance);
        }

        private void Register(Instance instance)
        {
            if (!_byName.TryGetValue(instance.Name, out var list))
            {
                list = new List<Instance>();
                _byName[instance.Name] = list;
            }
            list.Add(instance);
            _byGuid[instance.Guid] = instance;
        }

        private void Unregister(Instance instance)
        {
            if (_byName.TryGetValue(instance.Name, out var list))
            {
                list.Remove(instance);
                if (list.Count == 0)
                    _byName.Remove(instance.Name);
            }
            _byGuid.Remove(instance.Guid);
        }

        private void DestroyInstance(Instance instance)
        {
            // Destroy children first (bottom-up) so descendants are properly
            // unregistered before their ancestor disappears.
            foreach (var child in instance.Children.ToArray())
                DestroyInstance(child);

            instance.SetParent(null);
            Unregister(instance);
        }

        private void MoveInstance(Instance instance, Vector3Data position)
        {
            instance.SetProperty("Position", position);
        }

        /// <summary>
        /// Finds the Instance for a given Name and 1-based Index, ordered by
        /// creation time, per the documented GetGUID/Move/etc. contract.
        /// </summary>
        private Instance? Find(string name, int index)
        {
            if (index < 1)
                throw new ArgumentOutOfRangeException(nameof(index), "Index is 1-based and must be >= 1.");
            if (!_byName.TryGetValue(name, out var list))
                return null;
            return index <= list.Count ? list[index - 1] : null;
        }

        /// <summary>
        /// Resolves a Parent argument that may either be the name of a root
        /// container (e.g. "Workspace") or the name of an existing Instance.
        /// Returns null for root containers (no Instance parent), or the
        /// resolved Instance otherwise.
        /// </summary>
        private Instance? ResolveContainerOrInstance(string parent)
        {
            if (_rootContainers.Contains(parent))
                return null;

            var found = Find(parent, 1);
            if (found == null)
                throw new InvalidOperationException(
                    $"Parent '{parent}' is neither a known root container nor an existing Instance.");
            return found;
        }
    }
}
