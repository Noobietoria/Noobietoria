using System;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// A spatial Part (or one of its shape variants: Sphere, Wedge, Cylinder,
    /// MeshPart, Triangle, Mesh). Documented properties: RGBA (Color), Image
    /// Texture (NImageAssets), Size (XYZ) plus Position/Orientation.
    ///
    /// See: https://noobietoria.github.io/Docs/en/instance/#part
    /// </summary>
    public class Part : Instance
    {
        public Part(string name, string type, string rootContainer)
            : base(name, type, rootContainer)
        {
            SetProperty("Shape", string.Equals(type, "Part", StringComparison.OrdinalIgnoreCase)
                ? "Block"
                : type);
            SetProperty("Position", new Vector3Data(0, 0, 0));
            SetProperty("Size", new Vector3Data(1, 1, 1));
        }

        /// <summary>Shape of this Part: Block for plain Parts, otherwise the variant's own type name.</summary>
        public string Shape => GetProperty<string>("Shape", "Block") ?? "Block";

        public Vector3Data Position
        {
            get => GetProperty("Position", new Vector3Data(0, 0, 0));
            set => SetProperty("Position", value);
        }

        public Vector3Data Orientation
        {
            get => GetProperty("Orientation", new Vector3Data(0, 0, 0));
            set => SetProperty("Orientation", value);
        }

        public Vector3Data Size
        {
            get => GetProperty("Size", new Vector3Data(1, 1, 1));
            set => SetProperty("Size", value);
        }

        public ColorData Color
        {
            get => GetProperty("RGBA", new ColorData(1, 1, 1, 1));
            set => SetProperty("RGBA", value);
        }

        /// <summary>Texture asset from NImageAssets, or null when untinted.</summary>
        public string? Texture
        {
            get => GetProperty<string?>("Texture");
            set => SetProperty("Texture", value);
        }
    }

    /// <summary>
    /// Climbable truss — same properties as a Part plus ClimbSpeed.
    ///
    /// See: https://noobietoria.github.io/Docs/en/instance/#trusspart
    /// </summary>
    public class TrussPart : Part
    {
        public TrussPart(string name, string rootContainer)
            : base(name, "TrussPart", rootContainer)
        {
        }

        public double ClimbSpeed
        {
            get => GetProperty("ClimbSpeed", 0.0);
            set => SetProperty("ClimbSpeed", value);
        }
    }

    /// <summary>
    /// A named point with Position and Orientation, parented to a Part or
    /// Model for anchoring effects and attachments.
    ///
    /// See: https://noobietoria.github.io/Docs/en/instance/#attachment
    /// </summary>
    public class Attachment : Instance
    {
        public Attachment(string name, string rootContainer)
            : base(name, "Attachment", rootContainer)
        {
            SetProperty("Position", new Vector3Data(0, 0, 0));
            SetProperty("Orientation", new Vector3Data(0, 0, 0));
        }

        public Vector3Data Position
        {
            get => GetProperty("Position", new Vector3Data(0, 0, 0));
            set => SetProperty("Position", value);
        }

        public Vector3Data Orientation
        {
            get => GetProperty("Orientation", new Vector3Data(0, 0, 0));
            set => SetProperty("Orientation", value);
        }
    }
}
