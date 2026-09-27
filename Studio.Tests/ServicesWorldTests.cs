using System;
using System.Linq;
using Noobietoria.Studio.Core;
using Xunit;

namespace Noobietoria.Studio.Tests
{
    public class TweenServiceTests
    {
        [Fact]
        public void Tween_StateTransitions_WithInstanceResolution()
        {
            var instances = new InstanceService();
            var service = new TweenService(instances);
            instances.Create("Ball", "Part", "Workspace");

            service.Move("Ball", new Vector3Data(10, 0, 0), 2, "Out", "Quad");
            var tween = service.GetTween("Ball")!;
            Assert.Equal(TweenService.TweenState.Running, tween.State);
            Assert.Equal(new Vector3Data(0, 0, 0), tween.Start);
            Assert.Equal(new Vector3Data(10, 0, 0), tween.End);
            Assert.False(tween.Reversed);

            service.Pause("Ball");
            Assert.Equal(TweenService.TweenState.Paused, service.GetTween("Ball")!.State);
            Assert.Throws<InvalidOperationException>(() => service.Pause("Ball"));

            service.Resume("Ball");
            Assert.Equal(TweenService.TweenState.Running, service.GetTween("Ball")!.State);

            service.Back("Ball");
            Assert.True(service.GetTween("Ball")!.Reversed);
            Assert.Equal(new Vector3Data(0, 0, 0), service.GetTween("Ball")!.End);

            service.Stop("Ball");
            Assert.Equal(TweenService.TweenState.Stopped, service.GetTween("Ball")!.State);
        }

        [Fact]
        public void Move_UnknownTarget_Throws()
        {
            var service = new TweenService(new InstanceService());
            Assert.Throws<InvalidOperationException>(() =>
                service.Move("Ghost", new Vector3Data(1, 1, 1), 1));
            Assert.Throws<ArgumentException>(() =>
                service.Move("Ball", new Vector3Data(1, 1, 1), 0));
        }
    }

    public class LightingServiceTests
    {
        [Fact]
        public void Time_Weather_Ambient_Fog()
        {
            var service = new LightingService();

            service.SetTime("14:30:00");
            Assert.Equal("14:30:00", service.GetTime());
            Assert.Throws<ArgumentException>(() => service.SetTime("25:00:00"));
            Assert.Throws<ArgumentException>(() => service.SetTime("noon"));

            service.SetWeather("Storm", 0.7);
            Assert.Equal("Storm", service.GetWeather().Type);
            Assert.Equal(0.7, service.GetWeather().Intensity);
            Assert.Throws<ArgumentException>(() => service.SetWeather("Meteor"));
            Assert.Throws<ArgumentException>(() => service.SetWeather("Rain", 2));

            service.SetAmbient(new ColorData(0.2, 0.3, 0.4));
            Assert.Equal(new ColorData(0.2, 0.3, 0.4), service.GetProperty("Ambient"));

            service.SetFog(true, new ColorData(1, 1, 1), 10, 100);
            var fog = (LightingService.FogState)service.GetProperty("Fog")!;
            Assert.True(fog.Enabled);
            Assert.Equal(100, fog.End);
        }
    }

    public class CollisionServiceTests
    {
        [Fact]
        public void Groups_Assignment_AndCollisionMatrix()
        {
            var service = new CollisionService();
            service.CreateGroup("Players");
            service.CreateGroup("Hazards");
            Assert.Throws<InvalidOperationException>(() => service.CreateGroup("Players"));

            service.AssignGroup("Floor", "Players");
            service.AssignGroup("Trap", "Hazards");
            Assert.Equal("Players", service.GetGroup("Floor"));
            Assert.Equal("Hazards", service.GetGroup("Trap"));

            service.SetCollision("Players", "Hazards", false);
            Assert.False(service.Collides("Hazards", "Players")); // symmetric
            service.SetCollision("Players", "Hazards", true);
            Assert.True(service.Collides("Players", "Hazards"));

            Assert.Equal(new[] { "Hazards", "Players" }, service.GetGroups());

            service.DeleteGroup("Hazards"); // unassigns Trap
            Assert.Null(service.GetGroup("Trap"));
            Assert.NotNull(service.GetGroup("Floor"));
            Assert.Throws<InvalidOperationException>(() => service.SetCollision("Players", "Hazards", false));
        }
    }

    public class PhysicsServiceTests
    {
        [Fact]
        public void Gravity_Forces_AndBodyProperties()
        {
            var service = new PhysicsService();
            Assert.Equal(PhysicsService.DefaultGravity, service.GetGravity());

            service.SetGravity(100);
            Assert.Equal(100, service.GetGravity());

            service.SetVelocity("Ball", new Vector3Data(1, 2, 3));
            service.ApplyImpulse("Ball", new Vector3Data(0, 5, 0));
            Assert.Equal(new Vector3Data(1, 7, 3), service.GetVelocity("Ball"));

            service.ApplyForce("Ball", new Vector3Data(0, 10, 0), duration: 2);
            service.SetAnchored("Ball", true);
            service.SetMass("Ball", 4);
            service.SetFriction("Ball", 0.5);
            service.SetElasticity("Ball", 0.2);

            var snapshot = service.GetSnapshot("Ball");
            Assert.True(snapshot.Anchored);
            Assert.Equal(4, snapshot.Mass);
            Assert.Throws<ArgumentException>(() => service.SetMass("Ball", -1));
        }
    }

    public class EffectServiceTests
    {
        [Fact]
        public void Emit_Stop_ScreenEffects()
        {
            var service = new EffectService();
            service.Emit("Fire", new Vector3Data(1, 2, 3));
            service.EmitOn("Smoke", "Chimney");

            Assert.Equal(2, service.GetActiveEffects().Count);
            service.Stop("Fire");
            Assert.Single(service.GetActiveEffects());
            Assert.Equal("Chimney", service.GetActiveEffects().Single().Instance);

            service.ScreenEffect("Alice", "Flash", duration: 1);
            service.ScreenEffect("Alice", "Blur");
            Assert.Equal(2, service.GetScreenEffects("Alice").Count);
            service.ClearScreenEffect("Alice");
            Assert.Empty(service.GetScreenEffects("Alice"));

            service.StopAll();
            Assert.Empty(service.GetActiveEffects());
        }
    }

    public class SoundEffectServiceTests
    {
        [Fact]
        public void Play_SetVolume_Stop_Preload()
        {
            var service = new SoundEffectService();
            service.Play("explosion", new Vector3Data(1, 1, 1), volume: 0.8, rolloffDistance: 30);
            service.PlayOn("bgm", "Jukebox");

            Assert.Equal(2, service.GetActiveSounds().Count);
            service.SetVolume("explosion", 0.4);
            Assert.Equal(0.4, service.GetActiveSounds().First(s => s.SoundId == "explosion").Volume);

            service.Preload("explosion");
            Assert.True(service.IsPreloaded("explosion"));
            service.Release("explosion");
            Assert.False(service.IsPreloaded("explosion"));
            Assert.DoesNotContain(service.GetActiveSounds(), s => s.SoundId == "explosion");

            Assert.Throws<ArgumentException>(() => service.Play("boom", new Vector3Data(0, 0, 0), volume: 1.5));
            Assert.Throws<InvalidOperationException>(() => service.SetVolume("bgm-missing", 0.5));
        }
    }

    public class ProximityServiceTests
    {
        [Fact]
        public void PromptLifecycle_AndCallbacks()
        {
            var service = new ProximityService();
            service.Create("DoorPrompt", "Door", label: "Open", keyCode: "F", maxDistance: 5, holdDuration: 1);

            var prompt = service.GetPrompt("DoorPrompt")!;
            Assert.Equal("F", prompt.KeyCode);
            Assert.True(prompt.Visible && prompt.Enabled);

            int triggered = 0;
            service.OnTriggered("DoorPrompt", () => triggered++);
            service.GetCallbacks("DoorPrompt", "Triggered").ToList().ForEach(cb => cb());
            Assert.Equal(1, triggered);

            service.SetVisible("DoorPrompt", false);
            service.SetEnabled("DoorPrompt", false);
            Assert.False(service.GetPrompt("DoorPrompt")!.Visible);

            service.Remove("DoorPrompt");
            Assert.Null(service.GetPrompt("DoorPrompt"));
            Assert.Throws<InvalidOperationException>(() => service.Remove("DoorPrompt"));
        }
    }

    public class PathfindingServiceTests
    {
        [Fact]
        public void ComputePath_MoveTo_PauseResume()
        {
            var service = new PathfindingService(new InstanceService());
            int reached = 0;
            service.OnReached("Grunt", () => reached++);
            Assert.Single(service.ReachedCallbacks("Grunt"));

            var waypoints = service.ComputePath("Grunt", new Vector3Data(0, 0, 10));
            Assert.True(waypoints.Count >= 1);
            Assert.Equal(new Vector3Data(0, 0, 10), waypoints.Last());

            service.MoveTo("Grunt", new Vector3Data(0, 0, 5));
            Assert.Equal(PathfindingService.NavigationState.Moving, service.GetState("Grunt"));

            service.Pause("Grunt");
            Assert.Throws<InvalidOperationException>(() => service.Pause("Grunt"));
            service.Resume("Grunt");
            service.Stop("Grunt");
            Assert.Equal(PathfindingService.NavigationState.Stopped, service.GetState("Grunt"));
            Assert.Equal(0, reached); // navigation engine fires it in the real world
        }

        [Fact]
        public void MoveAlongWaypoints_AndInstanceDestination()
        {
            var instances = new InstanceService();
            var gate = (Part)instances.Create("Gate", "Part", "Workspace");
            gate.Position = new Vector3Data(3, 0, 3);

            var service = new PathfindingService(instances);
            var waypoints = service.ComputePath("Grunt", "Gate");
            Assert.Equal(new Vector3Data(3, 0, 3), waypoints.Last());

            service.MoveAlongWaypoints("Grunt", new[] { new Vector3Data(1, 0, 1), new Vector3Data(2, 0, 2) });
            Assert.Equal(2, service.GetWaypoints("Grunt").Count);
            Assert.Throws<ArgumentException>(() => service.MoveAlongWaypoints("Grunt", Array.Empty<Vector3Data>()));
        }
    }
}
