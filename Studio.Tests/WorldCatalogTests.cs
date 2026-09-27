using System;
using System.IO;
using System.Linq;
using Noobietoria.DedicatedServer;
using Xunit;

namespace Noobietoria.Studio.Tests
{
    public class WorldCatalogTests
    {
        [Fact]
        public void LoadsWorldDefinitionsFromDirectory()
        {
            string dir = Path.Combine(Path.GetTempPath(), "noobietoria-worlds-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(dir);
            try
            {
                File.WriteAllText(Path.Combine(dir, "grasslands.json"),
                    "{\"id\":\"grasslands\",\"name\":\"Grasslands Social\",\"description\":\"d1\"," +
                    "\"spawn\":{\"x\":1,\"y\":2,\"z\":3},\"spawnRadius\":5}");
                File.WriteAllText(Path.Combine(dir, "broken.json"), "{ not json");

                // Broken definitions are loud: the whole catalog fails so the
                // operator notices instead of silently hosting a half world.
                Assert.Throws<InvalidOperationException>(() => WorldCatalog.LoadFromDirectory(dir));
            }
            finally
            {
                Directory.Delete(dir, recursive: true);
            }
        }

        [Fact]
        public void GetOrFallback_ReturnsDefinition_OrSynthetic()
        {
            string dir = Path.Combine(Path.GetTempPath(), "noobietoria-worlds-" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(dir);
            try
            {
                File.WriteAllText(Path.Combine(dir, "grasslands.json"),
                    "{\"id\":\"grasslands\",\"name\":\"Grasslands\",\"description\":\"hills\"," +
                    "\"spawn\":{\"x\":0,\"y\":1,\"z\":0},\"spawnRadius\":6}");
                var catalog = WorldCatalog.LoadFromDirectory(dir);

                var world = catalog.GetOrFallback("grasslands");
                Assert.Equal("Grasslands", world.Name);
                Assert.Equal(new Noobietoria.Platform.WorldDefinition(
                    "grasslands", "Grasslands", "hills", 0, 1, 0, 6), world);

                var synthetic = catalog.GetOrFallback("moonbase");
                Assert.Equal("moonbase", synthetic.WorldId);
                Assert.Equal("moonbase", synthetic.Name);
                Assert.True(catalog.Contains("grasslands"));
                Assert.False(catalog.Contains("moonbase"));
            }
            finally
            {
                Directory.Delete(dir, recursive: true);
            }
        }

        [Fact]
        public void LoadFromDirectory_Missing_ReturnsEmpty()
        {
            var catalog = WorldCatalog.LoadFromDirectory(Path.Combine(Path.GetTempPath(), "does-not-exist-xyz"));
            Assert.Empty(catalog.Worlds);
        }
    }
}
