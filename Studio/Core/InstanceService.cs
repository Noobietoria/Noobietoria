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

        public void RegisterRootContainer(string name) => _rootContainers.Add(name);

        /// <summary>
        /// Every Instance type creatable through this service, per the
        /// Instance Reference docs. Useful for tooling (type pickers).
        /// </summary>
        public IReadOnlyList<string> GetKnownInstanceTypes() => InstanceCatalog.KnownTypeNames();

        /// <summary>
        /// InstanceService.Create(Name, Type, Parent, Properties). Type must
        /// be one of the documented types — see
        /// https://noobietoria.github.io/Docs/en/instance/.
        /// </summary>
        public Instance Create(string name, string type, string parent, IDictionary<string, object?>? properties = null)
        {
            if (string.IsNullOrWhiteSpace(name))
                throw new ArgumentException("Name must not be empty.", nameof(name));
            if (string.IsNullOrWhiteSpace(type))
                throw new ArgumentException("Type must not be empty.", nameof(type));
            if (string.IsNullOrWhiteSpace(parent))
                throw new ArgumentException("Parent must not be empty.", nameof(parent));
            if (!InstanceCatalog.TryGet(type, out var spec))
                throw new ArgumentException(
                    $"Unknown Instance type '{type}'. Known types: {string.Join(", ", InstanceCatalog.KnownTypeNames())}. "
                    + "See https://noobietoria.github.io/Docs/en/instance/.",
                    nameof(type));

            var parentInstance = ResolveContainerOrInstance(parent);
            string rootContainer = parentInstance is Instance pi ? pi.RootContainer : parent;

            var instance = InstanceCatalog.CreateInstance(name, spec.TypeName, rootContainer);

            if (properties != null)
            {
                foreach (var kv in properties)
                    instance.SetProperty(kv.Key, kv.Value);
            }

            // ServerScript / ClientScript / ModuleScript default to
            // Enabled = true unless explicitly overridden, per the docs.
            if (spec.Category == InstanceCategory.Script && !instance.Properties.ContainsKey("Enabled"))
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
        /// InstanceService.SetPivot(Name, Pivot, Index) — sets the world
        /// position of a Model's pivot. The Model must have PrimaryPart set
        /// first, per the Instance Reference docs.
        /// </summary>
        public void SetPivot(string name, Vector3Data pivot, int index = 1)
        {
            var model = RequireModel(name, index, nameof(SetPivot));
            RequirePrimaryPart(model, nameof(SetPivot));
            model.SetProperty("Pivot", pivot);
        }

        /// <summary>
        /// InstanceService.SetPivotByGUID(GUID, Pivot)
        /// </summary>
        public void SetPivotByGUID(string guid, Vector3Data pivot)
        {
            var model = RequireModelByGuid(guid, nameof(SetPivotByGUID));
            RequirePrimaryPart(model, nameof(SetPivotByGUID));
            model.SetProperty("Pivot", pivot);
        }

        /// <summary>
        /// InstanceService.MoveTo(Name, Position, Index) — moves every Part
        /// in the Model so its PrimaryPart lands exactly on Position. The
        /// Model must have PrimaryPart set first, per the docs.
        /// </summary>
        public void MoveTo(string name, Vector3Data position, int index = 1)
        {
            var model = RequireModel(name, index, nameof(MoveTo));
            RequirePrimaryPart(model, nameof(MoveTo));
            MoveModelTo(model, position);
        }

        /// <summary>
        /// InstanceService.MoveToByGUID(GUID, Position)
        /// </summary>
        public void MoveToByGUID(string guid, Vector3Data position)
        {
            var model = RequireModelByGuid(guid, nameof(MoveToByGUID));
            RequirePrimaryPart(model, nameof(MoveToByGUID));
            MoveModelTo(model, position);
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

        // ---- Model pivot / MoveTo internals --------------------------------

        private Model RequireModel(string name, int index, string methodName)
        {
            var instance = Find(name, index)
                ?? throw new InvalidOperationException($"No Instance named '{name}' at Index {index}.");
            return RequireModelInstance(instance, methodName);
        }

        private Model RequireModelByGuid(string guid, string methodName)
        {
            var instance = _byGuid.TryGetValue(guid, out var found)
                ? found
                : throw new InvalidOperationException($"No Instance with GUID '{guid}'.");
            return RequireModelInstance(instance, methodName);
        }

        private Model RequireModelInstance(Instance instance, string methodName)
        {
            if (instance is Model model)
                return model;
            throw new InvalidOperationException(
                $"InstanceService.{methodName} only applies to Model instances — '{instance.Name}' is a '{instance.Type}'.");
        }

        private static void RequirePrimaryPart(Model model, string methodName)
        {
            if (ResolvePrimaryPart(model) == null)
                throw new InvalidOperationException(
                    $"Model '{model.Name}' has no PrimaryPart — set Properties.PrimaryPart to a Part inside the Model "
                    + $"before calling InstanceService.{methodName} (see https://noobietoria.github.io/Docs/en/instance/#model).");
        }

        private static Instance? ResolvePrimaryPart(Model model)
        {
            string? reference = model.PrimaryPart;
            if (string.IsNullOrWhiteSpace(reference))
                return null;

            return model.DescendantsAndSelf().FirstOrDefault(d => string.Equals(d.Guid, reference, StringComparison.Ordinal))
                ?? model.DescendantsAndSelf().FirstOrDefault(d =>
                    string.Equals(d.Name, reference, StringComparison.Ordinal)
                    && !string.Equals(d.Type, "Model", StringComparison.OrdinalIgnoreCase));
        }

        private static void MoveModelTo(Model model, Vector3Data target)
        {
            var primary = ResolvePrimaryPart(model)!;
            var primaryPosition = primary.GetProperty("Position", new Vector3Data(0, 0, 0));
            var delta = new Vector3Data(
                target.X - primaryPosition.X,
                target.Y - primaryPosition.Y,
                target.Z - primaryPosition.Z);

            // Shift everything that has a Position: the PrimaryPart lands on
            // the target, every other Part/Attachment keeps its shape.
            foreach (var descendant in model.DescendantsAndSelf())
            {
                if (descendant.Properties.TryGetValue("Position", out var value) && value is Vector3Data position)
                    descendant.SetProperty("Position",
                        new Vector3Data(position.X + delta.X, position.Y + delta.Y, position.Z + delta.Z));
            }

            if (model.Pivot.HasValue)
            {
                var pivot = model.Pivot.Value;
                model.SetProperty("Pivot",
                    new Vector3Data(pivot.X + delta.X, pivot.Y + delta.Y, pivot.Z + delta.Z));
            }
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
