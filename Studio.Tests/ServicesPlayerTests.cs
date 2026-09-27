using System;
using System.Linq;
using Noobietoria.Studio.Core;
using Xunit;

namespace Noobietoria.Studio.Tests
{
    public class PlayerServiceTests
    {
        [Fact]
        public void JoinLeave_TrackPlayers_AndGetters()
        {
            var service = new PlayerService();
            var joined = new System.Collections.Generic.List<string>();
            var left = new System.Collections.Generic.List<string>();
            service.Joined += joined.Add;
            service.Left += left.Add;

            int aliceId = service.TrackJoin("Alice", country: "VN", avatar: "av-1", avatarUrl: "av-1.png",
                thumbnail: "th-1", thumbnailUrl: "th-1.png", character: "ch-1", characterUrl: "ch-1.png");
            service.TrackJoin("Bob");

            Assert.Equal(new[] { "Alice", "Bob" }, joined);
            Assert.Equal(new[] { "Alice", "Bob" }, service.GetPlayers());
            Assert.Equal(aliceId, service.GetPlayer("Alice")!.UserId);
            Assert.Equal("VN", service.GetPlayerCountry(aliceId));
            Assert.Equal("av-1", service.GetPlayerAvatar(aliceId));
            Assert.Equal("av-1.png", service.GetPlayerAvatarUrl(aliceId));
            Assert.Equal("th-1", service.GetPlayerThumbnail(aliceId));
            Assert.Equal("ch-1", service.GetPlayerCharacter(aliceId));
            Assert.Equal("Alice", service.GetPlayerName(aliceId));
            Assert.Null(service.GetPlayerName(999));

            service.TrackLeave("Alice");
            Assert.Equal<string>(new[] { "Alice" }, left);
            Assert.Equal(new[] { "Bob" }, service.GetPlayers());
            Assert.Null(service.GetPlayer("Alice"));
        }

        [Fact]
        public void GetCoors_And_Teleport_RoundTrip()
        {
            var service = new PlayerService();
            service.TrackJoin("Alice");

            Assert.Equal(new Vector3Data(0, 0, 0), service.GetCoors("Alice"));
            service.Teleport("Alice", new Vector3Data(5, 10, 15));
            Assert.Equal(new Vector3Data(5, 10, 15), service.GetCoors("Alice"));

            Assert.Throws<InvalidOperationException>(() => service.GetCoors("Ghost"));
            Assert.Throws<InvalidOperationException>(() => service.Teleport("Ghost", new Vector3Data(1, 1, 1)));
            Assert.Throws<InvalidOperationException>(() => service.TrackJoin("Alice"));
        }
    }

    public class SpawnServiceTests
    {
        [Fact]
        public void SpawnLifecycle_AndAssignments()
        {
            var service = new SpawnService();
            string? respawnedAt = null;
            service.Respawned += (target, username, spawnName, position) => respawnedAt = $"{target}:{username}:{spawnName}";

            service.CreateSpawn("Main", new Vector3Data(0, 1, 0));
            Assert.Throws<InvalidOperationException>(() => service.CreateSpawn("Main", new Vector3Data(0, 1, 0)));

            service.SetSpawn("Main", new Vector3Data(0, 2, 0));
            Assert.Equal(new Vector3Data(0, 2, 0), service.GetSpawn("Main"));
            Assert.Single(service.GetAllSpawns());

            service.AssignSpawn("Player", "Alice", "Main");
            service.Respawn("Player", "Alice");
            Assert.Equal("Player:Alice:Main", respawnedAt);

            service.RespawnAt("NPC", "ShopKeeper", "Main");
            Assert.Equal("NPC:ShopKeeper:Main", respawnedAt);

            Assert.Throws<ArgumentException>(() => service.AssignSpawn("Alien", "X", "Main"));
            Assert.Throws<InvalidOperationException>(() => service.Respawn("Player", "Bob"));
        }

        [Fact]
        public void DeleteSpawn_Missing_Throws()
        {
            var service = new SpawnService();
            Assert.Throws<InvalidOperationException>(() => service.DeleteSpawn("Nope"));
        }
    }

    public class TeleportServiceTests
    {
        [Fact]
        public void ZoneRegistration_AndTeleports()
        {
            var service = new TeleportService();
            TeleportService.TeleportEvent? seen = null;
            service.OnTeleport("Alice", e => seen = e);
            TeleportService.TeleportEvent? wildcard = null;
            service.OnTeleport("*", e => wildcard = e);

            service.RegisterZone("Dungeon", serverId: "srv-9", spawnPosition: new Vector3Data(1, 2, 3));
            Assert.Throws<InvalidOperationException>(() => service.RegisterZone("Dungeon"));

            service.TeleportToZone("Alice", "Dungeon");
            Assert.Equal("Dungeon", seen!.Destination);
            Assert.Equal("srv-9", seen.ServerId);
            Assert.Equal(new Vector3Data(1, 2, 3), seen.Position);
            Assert.NotNull(wildcard);

            service.TeleportToZone("Alice", "Dungeon", new Vector3Data(9, 9, 9));
            Assert.Equal(new Vector3Data(9, 9, 9), seen.Position);

            service.TeleportToServer("Alice", "srv-2");
            Assert.Equal("server:srv-2", seen.Destination);

            service.TeleportGroup(new[] { "A", "B" }, "Dungeon");
            Assert.Equal("Dungeon", wildcard!.Destination);

            Assert.Throws<InvalidOperationException>(() => service.TeleportToZone("Alice", "Nowhere"));
            Assert.Single(service.GetZones());
        }
    }

    public class CameraServiceTests
    {
        [Fact]
        public void CameraState_MergesAndResets()
        {
            var service = new CameraService();

            service.SetType("Alice", "follow"); // case-insensitive
            service.SetFOV("Alice", 70);
            service.SetPosition("Alice", new Vector3Data(0, 5, 10), new Vector3Data(0, 0, 0));
            service.LockTo("Alice", "Tower");
            service.Shake("Alice", 0.5, 2);

            var state = service.GetState("Alice")!;
            Assert.Equal("Follow", state.CameraType);
            Assert.Equal(70, state.FieldOfView);
            Assert.Equal("Tower", state.LockedTo);
            Assert.Equal(0.5, state.ShakeIntensity);

            service.Unlock("Alice");
            Assert.Null(service.GetState("Alice")!.LockedTo);

            service.Reset("Alice");
            Assert.Null(service.GetState("Alice"));

            service.SetType("*", "Fixed");
            Assert.Equal("Fixed", service.GetState("Bob")!.CameraType); // wildcard applies to everyone

            Assert.Throws<ArgumentException>(() => service.SetType("Alice", "Cinematic"));
            Assert.Throws<ArgumentException>(() => service.SetFOV("Alice", 0));
        }
    }
}
