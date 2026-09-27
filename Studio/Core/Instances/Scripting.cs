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
    /// Documented limits: payloads up to 64KB per call, and FireAllClients
    /// should not exceed 20 calls per second.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#networkevent
    /// </summary>
    public class NetworkEvent : Instance
    {
        /// <summary>Maximum serialized payload per call (64KB).</summary>
        public const long MaxPayloadBytes = 64 * 1024;

        /// <summary>Documented upper bound for FireAllClients calls per second.</summary>
        public const int MaxFireAllClientsPerSecond = 20;

        public NetworkEvent(string name, string rootContainer)
            : base(name, "NetworkEvent", rootContainer)
        {
        }
    }
}
