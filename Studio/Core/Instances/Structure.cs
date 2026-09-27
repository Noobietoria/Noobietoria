using System;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// Plain container Instance — only Name and Parent matter.
    ///
    /// See: https://noobietoria.github.io/Docs/en/instance/#folder
    /// </summary>
    public class Folder : Instance
    {
        public Folder(string name, string rootContainer)
            : base(name, "Folder", rootContainer)
        {
        }
    }

    /// <summary>
    /// Groups Parts into one logical object. Always set PrimaryPart (a Part
    /// inside the Model) before calling InstanceService.SetPivot or
    /// InstanceService.MoveTo — otherwise those calls fail, per the docs.
    ///
    /// See: https://noobietoria.github.io/Docs/en/instance/#model
    /// </summary>
    public class Model : Instance
    {
        public Model(string name, string rootContainer)
            : base(name, "Model", rootContainer)
        {
        }

        /// <summary>
        /// The Part used as the Model's coordinate origin — referenced by
        /// GUID or by Name of a Part inside this Model.
        /// </summary>
        public string? PrimaryPart
        {
            get => GetProperty<string?>("PrimaryPart");
            set => SetProperty("PrimaryPart", value);
        }

        /// <summary>World position of the Model's pivot (set via InstanceService.SetPivot).</summary>
        public Vector3Data? Pivot
        {
            get => Properties.TryGetValue("Pivot", out var value) && value is Vector3Data pivot
                ? pivot
                : null;
            set => SetProperty("Pivot", value);
        }
    }
}
