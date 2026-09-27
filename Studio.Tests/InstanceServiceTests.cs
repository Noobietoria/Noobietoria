using System;
using System.Collections.Generic;
using Noobietoria.Studio.Core;
using Xunit;

namespace Noobietoria.Studio.Tests
{
    public class InstanceServiceTests
    {
        [Fact]
        public void Create_ParentsUnderRootContainer()
        {
            var service = new InstanceService();

            var part = service.Create("Baseplate", "Part", "Workspace");

            Assert.Equal("Baseplate", part.Name);
            Assert.Equal("Part", part.Type);
            Assert.Equal("Workspace", part.RootContainer);
            Assert.Null(part.Parent);
        }

        [Fact]
        public void Create_ParentsUnderExistingInstance_InheritsRootContainer()
        {
            var service = new InstanceService();
            var folder = service.Create("Events", "Folder", "ReplicatedStorage");

            var networkEvent = service.Create("BuyItem", "NetworkEvent", "Events");

            Assert.Same(folder, networkEvent.Parent);
            Assert.Equal("ReplicatedStorage", networkEvent.RootContainer);
            Assert.Contains(networkEvent, folder.Children);
        }

        [Fact]
        public void Create_UnknownParent_Throws()
        {
            var service = new InstanceService();

            Assert.Throws<InvalidOperationException>(() =>
                service.Create("Part1", "Part", "DoesNotExist"));
        }

        [Fact]
        public void Create_ModuleScript_RequiresSourceProperty()
        {
            var service = new InstanceService();

            Assert.Throws<InvalidOperationException>(() =>
                service.Create("Utils", "ModuleScript", "ReplicatedStorage"));
        }

        [Fact]
        public void Create_ServerScript_DefaultsEnabledTrue()
        {
            var service = new InstanceService();

            var script = service.Create("Main", "ServerScript", "Workspace",
                new Dictionary<string, object?> { ["Source"] = "print('hi')" });

            Assert.True(service.IsEnabled("Main"));
        }

        [Fact]
        public void Create_ServerScript_CanStartDisabled()
        {
            var service = new InstanceService();

            service.Create("Main", "ServerScript", "Workspace",
                new Dictionary<string, object?> { ["Source"] = "print('hi')", ["Enabled"] = false });

            Assert.False(service.IsEnabled("Main"));
        }

        [Fact]
        public void GetGUID_DefaultsToFirstCreatedForSameName()
        {
            var service = new InstanceService();
            var first = service.Create("Part1", "Part", "Workspace");
            var second = service.Create("Part1", "Part", "Workspace");

            Assert.Equal(first.Guid, service.GetGUID("Part1"));
            Assert.Equal(second.Guid, service.GetGUID("Part1", 2));
        }

        [Fact]
        public void GetGUIDs_ReturnsAllSameNamedInstancesInCreationOrder()
        {
            var service = new InstanceService();
            var first = service.Create("Coin", "Part", "Workspace");
            var second = service.Create("Coin", "Part", "Workspace");
            var third = service.Create("Coin", "Part", "Workspace");

            var guids = service.GetGUIDs("Coin");

            Assert.Equal(new[] { first.Guid, second.Guid, third.Guid }, guids);
        }

        [Fact]
        public void GetByGUID_ReturnsCreatedInstance()
        {
            var service = new InstanceService();
            var part = service.Create("Part1", "Part", "Workspace");

            var resolved = service.GetByGUID(part.Guid);

            Assert.Same(part, resolved);
        }

        [Fact]
        public void Destroy_RemovesInstanceAndItsChildren()
        {
            var service = new InstanceService();
            var model = service.Create("Car", "Model", "Workspace");
            var part = service.Create("Body", "Part", "Car");

            service.Destroy("Car");

            Assert.Null(service.GetByGUID(model.Guid));
            Assert.Null(service.GetByGUID(part.Guid));
            Assert.Null(service.GetGUID("Car"));
            Assert.Null(service.GetGUID("Body"));
        }

        [Fact]
        public void DestroyByGUID_RemovesOnlyMatchingInstance()
        {
            var service = new InstanceService();
            var first = service.Create("Coin", "Part", "Workspace");
            var second = service.Create("Coin", "Part", "Workspace");

            service.DestroyByGUID(first.Guid);

            Assert.Null(service.GetByGUID(first.Guid));
            Assert.NotNull(service.GetByGUID(second.Guid));
            Assert.Single(service.GetGUIDs("Coin"));
        }

        [Fact]
        public void Move_UpdatesPositionProperty()
        {
            var service = new InstanceService();
            service.Create("Part1", "Part", "Workspace");

            service.Move("Part1", new Vector3Data(1, 2, 3));

            var instance = service.GetByGUID(service.GetGUID("Part1")!)!;
            var pos = instance.GetProperty<Vector3Data>("Position");
            Assert.Equal(1, pos.X);
            Assert.Equal(2, pos.Y);
            Assert.Equal(3, pos.Z);
        }

        [Fact]
        public void MoveByGUID_UpdatesPositionProperty()
        {
            var service = new InstanceService();
            var part = service.Create("Part1", "Part", "Workspace");

            service.MoveByGUID(part.Guid, new Vector3Data(4, 5, 6));

            var pos = part.GetProperty<Vector3Data>("Position");
            Assert.Equal(4, pos.X);
        }

        [Fact]
        public void Move_UnknownInstance_Throws()
        {
            var service = new InstanceService();

            Assert.Throws<InvalidOperationException>(() =>
                service.Move("Nope", new Vector3Data(0, 0, 0)));
        }

        [Fact]
        public void SetEnabled_And_IsEnabled_RoundTrip()
        {
            var service = new InstanceService();
            service.Create("Main", "ClientScript", "StarterPlayerScripts",
                new Dictionary<string, object?> { ["Source"] = "print('hi')" });

            service.SetEnabled("Main", false);

            Assert.False(service.IsEnabled("Main"));
        }

        [Fact]
        public void Require_NonModuleScript_Throws()
        {
            var service = new InstanceService();
            service.Create("Part1", "Part", "Workspace");
            var runtime = new FakeLuauRuntime();

            Assert.Throws<InvalidOperationException>(() =>
                service.Require(runtime, "Part1"));
        }

        [Fact]
        public void Require_ModuleScript_DelegatesToRuntime()
        {
            var service = new InstanceService();
            var module = service.Create("Utils", "ModuleScript", "ReplicatedStorage",
                new Dictionary<string, object?> { ["Source"] = "return {}" });
            var runtime = new FakeLuauRuntime();

            var result = service.Require(runtime, "Utils");

            Assert.Same(module, runtime.LastRequired);
            Assert.Equal("fake-module-value", result);
        }

        private sealed class FakeLuauRuntime : ILuauRuntime
        {
            public Instance? LastRequired { get; private set; }

            public object? RequireModule(Instance moduleScript)
            {
                LastRequired = moduleScript;
                return "fake-module-value";
            }
        }
    }
}
