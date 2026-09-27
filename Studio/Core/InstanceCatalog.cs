using System;
using System.Collections.Generic;
using System.Linq;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// Groups the documented Instance types by where they live in the engine:
    /// Spatial (Parts, TrussPart, Attachment), Structure (Folder, Model),
    /// Gui (Frame and friends), Messaging (NetworkEvent) and Script
    /// (ServerScript, ClientScript, ModuleScript).
    /// </summary>
    public enum InstanceCategory
    {
        Spatial,
        Structure,
        Gui,
        Messaging,
        Script,
    }

    /// <summary>
    /// Metadata for one creatable Instance type, as documented under
    /// https://noobietoria.github.io/Docs/en/instance/.
    /// </summary>
    public sealed record InstanceTypeSpec(string TypeName, InstanceCategory Category, string Summary);

    /// <summary>
    /// Catalog of every Instance type creatable through InstanceService, per
    /// the Instance Reference docs. Create() rejects anything outside this
    /// list; Studio tooling uses it to populate pickers.
    ///
    /// See: https://noobietoria.github.io/Docs/en/instance/
    /// </summary>
    public static class InstanceCatalog
    {
        private static readonly Dictionary<string, InstanceTypeSpec> TypesByName =
            new(StringComparer.OrdinalIgnoreCase);

        /// <summary>Every documented type, in documentation order.</summary>
        public static readonly IReadOnlyList<InstanceTypeSpec> Types =
            new List<InstanceTypeSpec>
            {
                // Spatial — Part and its shape variants (Part defaults to Block).
                new("Part", InstanceCategory.Spatial,
                    "Basic building block. Shape defaults to Block; RGBA, Texture, Size and Position/Orientation are documented properties."),
                new("Sphere", InstanceCategory.Spatial, "Part variant shaped as a sphere."),
                new("Wedge", InstanceCategory.Spatial, "Part variant shaped as a wedge."),
                new("Cylinder", InstanceCategory.Spatial, "Part variant shaped as a cylinder."),
                new("MeshPart", InstanceCategory.Spatial, "Part variant that renders a mesh asset."),
                new("Triangle", InstanceCategory.Spatial, "Part variant shaped as a triangle."),
                new("Mesh", InstanceCategory.Spatial, "Part variant that renders a mesh."),
                new("TrussPart", InstanceCategory.Spatial,
                    "Climbable truss; same properties as Part plus ClimbSpeed."),
                new("Attachment", InstanceCategory.Spatial,
                    "Named point with Position and Orientation, parented to a Part or Model."),

                // Structure
                new("Folder", InstanceCategory.Structure, "Plain container; only Name and Parent matter."),
                new("Model", InstanceCategory.Structure,
                    "Groups parts. Set PrimaryPart before calling InstanceService.SetPivot or MoveTo."),

                // Gui
                new("Frame", InstanceCategory.Gui, "Plain GUI panel (RGBA, Size XY)."),
                new("TextLabel", InstanceCategory.Gui, "Displays text (Text, TextSize, Font, TextColor3)."),
                new("TextButton", InstanceCategory.Gui, "Clickable text element."),
                new("ImageLabel", InstanceCategory.Gui,
                    "Displays an image from NImageAssets (Image, ImageColor3, ImageTransparency, ScaleType)."),
                new("ImageButton", InstanceCategory.Gui, "Clickable image element."),
                new("TextBox", InstanceCategory.Gui,
                    "Editable text (PlaceholderText, ClearTextOnFocus, MultiLine)."),
                new("ScrollingFrame", InstanceCategory.Gui,
                    "Frame with scrolling (CanvasSize, ScrollBarThickness, ScrollingEnabled, ElasticBehavior)."),

                // Messaging
                new("NetworkEvent", InstanceCategory.Messaging,
                    "Client/server messaging primitive; the Name identifies it in code."),

                // Script
                new("ServerScript", InstanceCategory.Script,
                    "Luau code that runs on the server (Properties.Source, Enabled)."),
                new("ClientScript", InstanceCategory.Script,
                    "Luau code that runs on the client (Properties.Source, Enabled)."),
                new("ModuleScript", InstanceCategory.Script,
                    "Luau module whose Source must return a value; loaded via require."),
            }.AsReadOnly();

        static InstanceCatalog()
        {
            foreach (var spec in Types)
                TypesByName.Add(spec.TypeName, spec);
        }

        /// <summary>True when the type name is documented (case-insensitive).</summary>
        public static bool IsKnown(string typeName) =>
            typeName != null && TypesByName.ContainsKey(typeName);

        public static bool TryGet(string typeName, out InstanceTypeSpec spec) =>
            TypesByName.TryGetValue(typeName ?? string.Empty, out spec!);

        /// <summary>All documented type names, in documentation order.</summary>
        public static IReadOnlyList<string> KnownTypeNames() =>
            Types.Select(t => t.TypeName).ToArray();

        /// <summary>
        /// Creates the concrete Instance for a known type name. The type must
        /// already be validated via <see cref="IsKnown"/>.
        /// </summary>
        internal static Instance CreateInstance(string name, string typeName, string rootContainer)
        {
            var spec = TypesByName[typeName];
            return spec.Category switch
            {
                InstanceCategory.Spatial when string.Equals(spec.TypeName, "TrussPart", StringComparison.OrdinalIgnoreCase)
                    => new TrussPart(name, rootContainer),
                InstanceCategory.Spatial when string.Equals(spec.TypeName, "Attachment", StringComparison.OrdinalIgnoreCase)
                    => new Attachment(name, rootContainer),
                InstanceCategory.Spatial => new Part(name, spec.TypeName, rootContainer),

                InstanceCategory.Structure when string.Equals(spec.TypeName, "Model", StringComparison.OrdinalIgnoreCase)
                    => new Model(name, rootContainer),
                InstanceCategory.Structure => new Folder(name, rootContainer),

                InstanceCategory.Gui => spec.TypeName.ToLowerInvariant() switch
                {
                    "frame" => new Frame(name, rootContainer),
                    "textlabel" => new TextLabel(name, rootContainer),
                    "textbutton" => new TextButton(name, rootContainer),
                    "imagelabel" => new ImageLabel(name, rootContainer),
                    "imagebutton" => new ImageButton(name, rootContainer),
                    "textbox" => new TextBox(name, rootContainer),
                    _ => new ScrollingFrame(name, rootContainer),
                },

                InstanceCategory.Messaging => new NetworkEvent(name, rootContainer),
                _ => new ScriptInstance(name, spec.TypeName, rootContainer),
            };
        }
    }

    /// <summary>
    /// RGBA color with channels in the 0..1 range, mirroring the docs' RGBA
    /// property shape.
    /// </summary>
    public readonly struct ColorData
    {
        public double R { get; }
        public double G { get; }
        public double B { get; }
        public double A { get; }

        public ColorData(double r, double g, double b, double a = 1.0)
        {
            R = r;
            G = g;
            B = b;
            A = a;
        }
    }
}
