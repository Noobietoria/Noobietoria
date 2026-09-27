using System;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// Base for the Luau script Instances (ServerScript, ClientScript,
    /// ModuleScript): Luau source lives in Properties.Source and Enabled
    /// defaults to true.
    ///
    /// See: https://noobietoria.github.io/Docs/en/instance/#serverscript
    /// </summary>
    public class ScriptInstance : Instance
    {
        public ScriptInstance(string name, string type, string rootContainer)
            : base(name, type, rootContainer)
        {
        }

        /// <summary>Luau source code of the script.</summary>
        public string? Source
        {
            get => GetProperty<string?>("Source");
            set => SetProperty("Source", value);
        }

        /// <summary>Whether the script runs; defaults to true per the docs.</summary>
        public bool Enabled
        {
            get => GetProperty("Enabled", true);
            set => SetProperty("Enabled", value);
        }
    }

    /// <summary>
    /// Client/server messaging primitive; the Name identifies the event in
    /// code. Recommend parenting under ReplicatedStorage so both sides can
    /// reach it.
    ///
    /// See: https://noobietoria.github.io/Docs/en/instance/#networkevent
    /// </summary>
    public class NetworkEvent : Instance
    {
        public NetworkEvent(string name, string rootContainer)
            : base(name, "NetworkEvent", rootContainer)
        {
        }
    }
}
