using System;
using System.Linq;
using Noobietoria.Studio.Core;
using Xunit;

namespace Noobietoria.Studio.Tests
{
    public class InstanceTypesTests
    {
        private static readonly string[] AllDocumentedTypes =
        {
            "Part", "Sphere", "Wedge", "Cylinder", "MeshPart", "Triangle", "Mesh",
            "TrussPart", "Attachment",
            "Folder", "Model",
            "Frame", "TextLabel", "TextButton", "ImageLabel", "ImageButton", "TextBox", "ScrollingFrame",
            "NetworkEvent",
            "ServerScript", "ClientScript", "ModuleScript",
        };

        [Theory]
        [MemberData(nameof(AllTypes))]
        public void Create_SupportsEveryDocumentedType(string type)
        {
            var service = new InstanceService();
            var properties = new System.Collections.Generic.Dictionary<string, object?>();

            // ModuleScript's documented contract: Source must return a value.
            if (type == "ModuleScript")
                properties["Source"] = "return 42";

            var instance = service.Create("Test" + type, type, "Workspace", properties);

            Assert.Equal(type, instance.Type);
            Assert.Equal("Workspace", instance.RootContainer);
        }

        public static System.Collections.Generic.IEnumerable<object[]> AllTypes()
            => AllDocumentedTypes.Select(t => new object[] { t });

        [Fact]
        public void Create_IsCaseInsensitive_OnTypeNames()
        {
            var service = new InstanceService();

            var part = service.Create("Floor", "part", "Workspace");

            Assert.Equal("Part", part.Type);
        }

        [Fact]
        public void Create_UnknownType_ThrowsWithKnownList()
        {
            var service = new InstanceService();

            var error = Assert.Throws<ArgumentException>(() =>
                service.Create("Thing", "Teleporter", "Workspace"));

            Assert.Contains("Unknown Instance type 'Teleporter'", error.Message);
            Assert.Contains("ModuleScript", error.Message);
        }

        [Fact]
        public void Part_HasDocumentedDefaultsAndTypedAccessors()
        {
            var service = new InstanceService();

            var part = (Part)service.Create("Floor", "Part", "Workspace");

            Assert.Equal("Block", part.Shape);
            Assert.Equal(new Vector3Data(0, 0, 0), part.Position);
            Assert.Equal(new Vector3Data(1, 1, 1), part.Size);
            Assert.Equal(new ColorData(1, 1, 1, 1), part.Color);

            part.Size = new Vector3Data(4, 1, 2);
            part.Color = new ColorData(0.2, 0.4, 0.6, 0.8);
            part.Texture = "wood-plank";
            part.Position = new Vector3Data(1, 2, 3);

            Assert.Equal(new Vector3Data(4, 1, 2), part.Size);
            Assert.Equal(new ColorData(0.2, 0.4, 0.6, 0.8), part.Color);
            Assert.Equal("wood-plank", part.Texture);
            Assert.Equal(new Vector3Data(1, 2, 3), part.Position);

            service.Move("Floor", new Vector3Data(9, 9, 9));
            Assert.Equal(new Vector3Data(9, 9, 9), part.Position);
        }

        [Fact]
        public void Part_ShapeVariants_KeepTheirOwnShape()
        {
            var service = new InstanceService();

            Assert.Equal("Sphere", ((Part)service.Create("Ball", "Sphere", "Workspace")).Shape);
            Assert.Equal("Wedge", ((Part)service.Create("Ramp", "Wedge", "Workspace")).Shape);
            Assert.Equal("Cylinder", ((Part)service.Create("Pole", "Cylinder", "Workspace")).Shape);
        }

        [Fact]
        public void TrussPart_ExposeClimbSpeed()
        {
            var service = new InstanceService();

            var truss = (TrussPart)service.Create("Ladder", "TrussPart", "Workspace");

            Assert.Equal(0.0, truss.ClimbSpeed);
            truss.ClimbSpeed = 6.5;
            Assert.Equal(6.5, truss.ClimbSpeed);
        }

        [Fact]
        public void Attachment_RoundTripsPositionAndOrientation()
        {
            var service = new InstanceService();
            service.Create("Base", "Part", "Workspace");

            var attachment = (Attachment)service.Create("Grip", "Attachment", "Base");

            attachment.Position = new Vector3Data(0, 2, 0);
            attachment.Orientation = new Vector3Data(0, 90, 0);

            Assert.Equal(new Vector3Data(0, 2, 0), attachment.Position);
            Assert.Equal(new Vector3Data(0, 90, 0), attachment.Orientation);
        }

        [Fact]
        public void ScriptInstances_ExposeSourceAndEnabled()
        {
            var service = new InstanceService();

            var server = (ScriptInstance)service.Create("Main", "ServerScript", "Workspace",
                new System.Collections.Generic.Dictionary<string, object?> { ["Source"] = "print('hi')" });

            Assert.Equal("print('hi')", server.Source);
            Assert.True(server.Enabled);

            server.Enabled = false;
            Assert.False(service.IsEnabled("Main"));
        }

        [Fact]
        public void GuiInstances_ExposeDocumentedProperties()
        {
            var service = new InstanceService();

            var label = (TextLabel)service.Create("Title", "TextLabel", "Workspace");
            label.Text = "Hello";
            label.TextColor3 = new ColorData(1, 0, 0);

            var image = (ImageLabel)service.Create("Icon", "ImageLabel", "Workspace");
            image.Image = "coin-icon";
            image.ImageTransparency = 0.25;

            var scroll = (ScrollingFrame)service.Create("List", "ScrollingFrame", "Workspace");
            scroll.ScrollingEnabled = false;

            Assert.Equal("Hello", label.Text);
            Assert.Equal(new ColorData(1, 0, 0), label.TextColor3);
            Assert.Equal("coin-icon", image.Image);
            Assert.Equal(0.25, image.ImageTransparency);
            Assert.False(scroll.ScrollingEnabled!.Value);
        }

        // ---- Model SetPivot / MoveTo ----

        [Fact]
        public void SetPivot_And_MoveTo_RequirePrimaryPart()
        {
            var service = new InstanceService();
            service.Create("House", "Model", "Workspace");

            Assert.Throws<InvalidOperationException>(() =>
                service.SetPivot("House", new Vector3Data(1, 1, 1)));
            Assert.Throws<InvalidOperationException>(() =>
                service.MoveTo("House", new Vector3Data(1, 1, 1)));
        }

        [Fact]
        public void SetPivot_And_MoveTo_RejectNonModels()
        {
            var service = new InstanceService();
            service.Create("Floor", "Part", "Workspace");

            Assert.Throws<InvalidOperationException>(() =>
                service.SetPivot("Floor", new Vector3Data(1, 1, 1)));
            Assert.Throws<InvalidOperationException>(() =>
                service.MoveTo("Floor", new Vector3Data(1, 1, 1)));
        }

        [Fact]
        public void SetPivot_And_MoveTo_ThrowForUnknownInstances()
        {
            var service = new InstanceService();

            Assert.Throws<InvalidOperationException>(() =>
                service.SetPivot("Missing", new Vector3Data(1, 1, 1)));
            Assert.Throws<InvalidOperationException>(() =>
                service.MoveTo("Missing", new Vector3Data(1, 1, 1)));
        }

        [Fact]
        public void MoveTo_LandsPrimaryPartOnTarget_AndShiftsSiblings()
        {
            var service = new InstanceService();

            var model = (Model)service.Create("House", "Model", "Workspace");
            var wall = (Part)service.Create("Wall", "Part", "House");
            var door = (Part)service.Create("Door", "Part", "House");
            door.Position = new Vector3Data(0, 0, 3);

            model.PrimaryPart = wall.Guid; // PrimaryPart referenced by GUID

            service.MoveTo("House", new Vector3Data(10, 5, 10));

            Assert.Equal(new Vector3Data(10, 5, 10), wall.Position);
            Assert.Equal(new Vector3Data(10, 5, 13), door.Position);
        }

        [Fact]
        public void MoveTo_ResolvesPrimaryPartByName()
        {
            var service = new InstanceService();

            var model = (Model)service.Create("House", "Model", "Workspace");
            var wall = (Part)service.Create("Wall", "Part", "House");
            wall.Position = new Vector3Data(2, 0, 2);
            model.PrimaryPart = "Wall"; // PrimaryPart referenced by Name

            service.MoveTo("House", new Vector3Data(4, 0, 4));

            Assert.Equal(new Vector3Data(4, 0, 4), wall.Position);
        }

        [Fact]
        public void SetPivot_Then_MoveTo_ShiftsPivotBySameDelta()
        {
            var service = new InstanceService();

            var model = (Model)service.Create("House", "Model", "Workspace");
            var wall = (Part)service.Create("Wall", "Part", "House");
            model.PrimaryPart = wall.Guid;

            service.SetPivot("House", new Vector3Data(1, 1, 1));
            service.MoveTo("House", new Vector3Data(10, 5, 10));

            Assert.Equal(new Vector3Data(11, 6, 11), model.Pivot!.Value);
            Assert.Equal(new Vector3Data(10, 5, 10), wall.Position);
        }

        [Fact]
        public void SetPivotByGUID_And_MoveToByGUID_Work()
        {
            var service = new InstanceService();

            var model = (Model)service.Create("House", "Model", "Workspace");
            var wall = (Part)service.Create("Wall", "Part", "House");
            model.PrimaryPart = wall.Guid;

            service.SetPivotByGUID(model.Guid, new Vector3Data(1, 2, 3));
            Assert.Equal(new Vector3Data(1, 2, 3), model.Pivot!.Value);

            service.MoveToByGUID(model.Guid, new Vector3Data(5, 6, 7));
            Assert.Equal(new Vector3Data(5, 6, 7), wall.Position);
        }

        [Fact]
        public void GetKnownInstanceTypes_ListsEveryDocumentedType()
        {
            var service = new InstanceService();

            var known = service.GetKnownInstanceTypes();

            foreach (string type in AllDocumentedTypes)
                Assert.Contains(type, known);
        }
    }
}
