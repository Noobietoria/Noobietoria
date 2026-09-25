namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// Abstraction over whatever Luau runtime Studio embeds (e.g. a native
    /// Luau VM binding). InstanceService depends only on this interface so
    /// the actual scripting engine integration can land in a follow-up PR
    /// without reshaping the Instance/InstanceService contract.
    /// </summary>
    public interface ILuauRuntime
    {
        /// <summary>
        /// Runs a ModuleScript's Source exactly once and returns (and caches)
        /// its returned value, per the ModuleScript contract in the docs.
        /// </summary>
        object? RequireModule(Instance moduleScript);
    }
}
