using System;
using System.Collections.Generic;
using System.Linq;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// Base class for every object that can be created, parented, moved and
    /// destroyed through <see cref="InstanceService"/>.
    ///
    /// This mirrors the "Instance" concept described in the public Studio API
    /// docs (Part, Model, Folder, NetworkEvent, ServerScript, ClientScript,
    /// ModuleScript, ...). Concrete instance types live under
    /// Studio/Core/Instances and derive from this class.
    /// </summary>
    public class Instance
    {
        /// <summary>
        /// Unique identifier assigned at creation time. Used by InstanceService
        /// whenever multiple Instances share the same Name.
        /// </summary>
        public string Guid { get; } = System.Guid.NewGuid().ToString("N");

        /// <summary>
        /// Display / lookup name (not guaranteed unique).
        /// </summary>
        public string Name { get; set; }

        /// <summary>
        /// Instance type as used by the public API, e.g. "Part", "Model",
        /// "Folder", "NetworkEvent", "ServerScript", "ClientScript",
        /// "ModuleScript".
        /// </summary>
        public string Type { get; }

        /// <summary>
        /// Parent Instance, or null if this Instance is parented directly to a
        /// root container (Workspace, ReplicatedStorage, ServerStorage, ...).
        /// </summary>
        public Instance? Parent { get; private set; }

        /// <summary>
        /// Name of the root container this Instance lives under
        /// (Workspace, ReplicatedStorage, ServerStorage, StarterPlayerScripts, ...).
        /// Set on creation and kept even if the Instance is later re-parented
        /// to another Instance, so it always resolves back to a root.
        /// </summary>
        public string RootContainer { get; internal set; }

        /// <summary>
        /// Children currently parented to this Instance.
        /// </summary>
        public IReadOnlyList<Instance> Children => _children;
        private readonly List<Instance> _children = new();

        /// <summary>
        /// Free-form property bag (Position, Source, Enabled, Size, RGBA, ...).
        /// Kept generic here; typed accessors are added on concrete subclasses.
        /// </summary>
        public Dictionary<string, object?> Properties { get; } = new();

        /// <summary>
        /// Timestamp used by InstanceService to order same-named Instances
        /// (Index = 1 is the oldest).
        /// </summary>
        public DateTime CreatedAt { get; } = DateTime.UtcNow;

        public Instance(string name, string type, string rootContainer)
        {
            if (string.IsNullOrWhiteSpace(name))
                throw new ArgumentException("Instance name must not be empty.", nameof(name));
            if (string.IsNullOrWhiteSpace(type))
                throw new ArgumentException("Instance type must not be empty.", nameof(type));

            Name = name;
            Type = type;
            RootContainer = rootContainer;
        }

        internal void SetParent(Instance? parent)
        {
            Parent?._children.Remove(this);
            Parent = parent;
            parent?._children.Add(this);
        }

        public T? GetProperty<T>(string key, T? defaultValue = default)
        {
            if (Properties.TryGetValue(key, out var value) && value is T typed)
                return typed;
            return defaultValue;
        }

        public void SetProperty(string key, object? value) => Properties[key] = value;

        /// <summary>
        /// Depth-first enumeration of this Instance and all of its descendants.
        /// </summary>
        public IEnumerable<Instance> DescendantsAndSelf()
        {
            yield return this;
            foreach (var child in _children.ToArray())
                foreach (var descendant in child.DescendantsAndSelf())
                    yield return descendant;
        }
    }
}
