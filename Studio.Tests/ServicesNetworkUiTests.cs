using System;
using System.Linq;
using Noobietoria.Studio.Core;
using Xunit;

namespace Noobietoria.Studio.Tests
{
    public class LuauValuesTests
    {
        [Theory]
        [InlineData(true)]
        public void IsSerializable_AcceptsPrimitivesTablesAndLists(object? value)
        {
            Assert.True(LuauValues.IsSerializable(value));
        }

        [Fact]
        public void IsSerializable_AcceptsNestedTables()
        {
            Assert.True(LuauValues.IsSerializable(new Dictionary<string, object?>
            {
                ["name"] = "Alice",
                ["level"] = 7,
                ["vip"] = true,
                ["stats"] = new Dictionary<string, object?> { ["hp"] = 100 },
                ["tags"] = new object?[] { "a", "b" },
            }));
        }

        [Fact]
        public void IsSerializable_RejectsNonPrimitives()
        {
            Assert.False(LuauValues.IsSerializable(new object()));
            Assert.False(LuauValues.IsSerializable(new Dictionary<string, object?> { ["bad"] = new object() }));
        }

        [Fact]
        public void EnsureSerializable_ThrowsWithMessage()
        {
            Assert.Throws<ArgumentException>(() => LuauValues.EnsureSerializable(new object(), "payload"));
        }
    }

    public class NetworkServiceTests
    {
        [Fact]
        public void Create_FireClient_FireServer_RoundTrip()
        {
            var instances = new InstanceService();
            var hub = new NetworkHub();
            var server = new NetworkService(instances, NetworkService.RpcSide.Server, hub);
            var client = new NetworkService(instances, NetworkService.RpcSide.Client, hub);

            server.Create("BuyItem");
            Assert.NotNull(instances.GetGUID("BuyItem"));

            object?[]? received = null;
            server.OnServerEvent("BuyItem", (username, args) => received = args);

            client.FireServer("BuyItem", "sword", 2);
            Assert.NotNull(received);
            Assert.Equal("sword", received![0]);
            Assert.Equal(2, received[1]);

            object?[]? clientReceived = null;
            client.OnClientEvent("BuyItem", args => clientReceived = args);
            server.FireClient("BuyItem", "Alice", "ok");
            Assert.NotNull(clientReceived);

            server.FireAllClients("BuyItem", "hello-all");
            Assert.Equal("hello-all", clientReceived![0]);

            client.Disconnect("BuyItem");
            server.FireClient("BuyItem", "Alice", "again");
            Assert.Equal("hello-all", clientReceived[0]); // not updated
        }

        [Fact]
        public void SideRules_AreEnforced()
        {
            var server = new NetworkService(new InstanceService(), NetworkService.RpcSide.Server);
            var client = new NetworkService(new InstanceService(), NetworkService.RpcSide.Client);

            Assert.Throws<InvalidOperationException>(() => client.FireAllClients("E", 1));
            Assert.Throws<InvalidOperationException>(() => server.FireServer("E", 1));
            Assert.Throws<InvalidOperationException>(() => client.OnServerEvent("E", (_, _) => { }));
            Assert.Throws<InvalidOperationException>(() => server.OnClientEvent("E", _ => { }));
        }

        [Fact]
        public void Payloads_MustBeSerializable()
        {
            var service = new NetworkService();
            Assert.Throws<ArgumentException>(() => service.FireAllClients("E", new object()));
            Assert.Throws<ArgumentException>(() => service.FireAllClients("E", new Dictionary<string, object?> { ["bad"] = new object() }));
        }

        [Fact]
        public void Destroy_RemovesCallbacks()
        {
            var instances = new InstanceService();
            var service = new NetworkService(instances);
            service.Create("Chat", index: 2);
            service.OnServerEvent("Chat", (_, _) => { }, index: 2);
            service.Destroy("Chat", index: 2);
            Assert.Null(instances.GetGUID("Chat#2"));
        }
    }

    public class NetworkEventServiceTests
    {
        [Fact]
        public void InvokeServer_RoundTrip()
        {
            var hub = new NetworkEventHub();
            var server = new NetworkEventService(NetworkService.RpcSide.Server, hub);
            var client = new NetworkEventService(NetworkService.RpcSide.Client, hub);

            server.OnServerInvoke("GetCoins", args => 42);
            Assert.Equal(42, client.InvokeServer("GetCoins"));

            Assert.Throws<InvalidOperationException>(() => client.InvokeServer("Unknown"));
        }

        [Fact]
        public void FireAllClientsExcept_SkipsOwner()
        {
            var hub = new NetworkEventHub();
            var server = new NetworkEventService(NetworkService.RpcSide.Server, hub);
            var alice = new NetworkEventService(NetworkService.RpcSide.Client, hub);
            var bob = new NetworkEventService(NetworkService.RpcSide.Client, hub);

            object?[]? aliceGot = null, bobGot = null;
            alice.OnClientEvent("Chat", args => aliceGot = args, owner: "Alice");
            bob.OnClientEvent("Chat", args => bobGot = args, owner: "Bob");

            server.FireAllClientsExcept("Chat", "Alice", "hi");
            Assert.Null(aliceGot);
            Assert.NotNull(bobGot);

            server.Declare("Chat");
            Assert.True(server.Exists("Chat"));
            server.Disconnect("Chat");
        }
    }

    public class UIServiceTests
    {
        [Fact]
        public void Create_TypedSetters_AndCallbacks()
        {
            var service = new UIService();
            service.Create("Root", "Frame", "ScreenGui");
            service.Create("Title", "TextLabel", "Root");
            service.Create("Icon", "ImageLabel", "Root");
            service.Create("List", "ScrollingFrame", "Root");
            Assert.Throws<InvalidOperationException>(() => service.Create("Root", "Frame", "ScreenGui"));
            Assert.Throws<ArgumentException>(() => service.Create("Bad", "VideoFrame", "ScreenGui"));

            int clicks = 0;
            service.OnClick("Title", () => clicks++);
            service.FireClick("Title");
            Assert.Equal(1, clicks);

            string? text = null;
            service.OnTextChanged("Title", t => text = t);
            service.FireTextChanged("Title", "New");
            Assert.Equal("New", text);
            Assert.Equal("New", service.GetText("Title"));

            service.SetImage("Icon", "coin-64");
            Assert.Equal("coin-64", service.GetProperty("Icon", "Image"));

            // SetPlaceholder on non-TextBox throws:
            Assert.Throws<InvalidOperationException>(() => service.SetPlaceholder("Title", "x"));

            service.SetCanvasSize("List", 200, 400);
            Assert.Equal(new Vector3Data(200, 400, 0), service.GetCanvasSize("List"));
            service.SetScrollPosition("List", 10, 20);
            Assert.Equal(new Vector3Data(10, 20, 0), service.GetScrollPosition("List"));

            service.Tween("Title", new Dictionary<string, object?> { ["Position"] = "1,1" }, 0.5, "Out", "Quad");
            Assert.Single(service.GetTweens());

            service.RemoveCallback("Title");
            service.SetVisible("Title", false);
            Assert.False(service.GetElement("Title")!.Visible);
            service.Destroy("Title");
            Assert.Null(service.GetElement("Title"));
        }
    }

    public class InputServiceTests
    {
        [Fact]
        public void Bindings_AndFiring()
        {
            var service = new InputService();
            int pressed = 0;
            service.OnKeyPress("Alice", "E", () => pressed++);
            service.OnKeyRelease("Alice", "E", () => pressed += 10);
            service.OnMouseClick("Alice", "Left", () => pressed += 100);
            service.OnMouseMove("Alice", () => pressed += 1000);
            service.OnTouch("Alice", () => pressed += 10000);

            service.FireKeyPress("Alice", "E");
            service.FireKeyRelease("Alice", "E");
            service.FireMouseClick("Alice", "left"); // case-insensitive
            service.FireMouseMove("Alice");
            service.FireTouch("Alice");
            Assert.Equal(11111, pressed);

            service.BindAction("Dash", new[] { "Space", "LeftShift" }, () => { });
            Assert.True(service.IsBound("Dash"));
            service.UnbindAction("Dash");
            Assert.False(service.IsBound("Dash"));
            Assert.Throws<InvalidOperationException>(() => service.UnbindAction("Dash"));

            Assert.Throws<ArgumentException>(() => service.OnMouseClick("Alice", "Side", () => { }));
        }
    }

    public class TextServiceTests
    {
        [Fact]
        public void Measure_Wrap_Truncate_Escape()
        {
            var service = new TextService();
            var metrics = service.MeasureText("Hello", fontSize: 10);
            Assert.Equal(30, metrics.Width); // 5 chars x 10 x 0.6

            Assert.True(service.FitText("Hi", 100));
            Assert.False(service.FitText("Hello world big", 20));

            var lines = service.WrapText("one two three four", 100, fontSize: 10);
            Assert.Contains("one two", lines.First());

            Assert.Equal("a\nb…", service.Truncate("a\nb\nc", 2));
            Assert.Equal("a\nb\nc", service.Truncate("a\nb\nc", 3));

            Assert.Equal("&lt;b&gt; &amp; x", service.EscapeRichText("<b> & x"));
            Assert.Equal(5, service.CountCharacters("Hello"));
            Assert.Equal("unchanged", service.Filter("Alice", "unchanged"));
        }
    }

    public class NotificationServiceTests
    {
        [Fact]
        public void Send_Dismiss_OnDismiss()
        {
            var service = new NotificationService();
            int dismissed = 0;
            service.OnDismiss("Alice", id => dismissed++);

            int id = service.Send("Alice", "Welcome", "body", "Popup", 10);
            service.SendAll("Server", "restart", "Banner", 30);
            Assert.Equal(2, service.GetActive("Alice").Count);

            service.Dismiss("Alice", id);
            Assert.Equal(1, dismissed);
            Assert.Single(service.GetActive("Alice"));

            Assert.Throws<InvalidOperationException>(() => service.Dismiss("Alice", id));
            Assert.Throws<ArgumentException>(() => service.Send("Alice", "t", "m", "Laser"));
        }
    }
}
