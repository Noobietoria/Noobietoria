using System;
using System.Collections.Generic;
using Noobietoria.DedicatedServer;
using Noobietoria.Studio.Core;
using Xunit;

namespace Noobietoria.Studio.Tests
{
    public class ServerDedicatedServerTests
    {
        [Fact]
        public void Start_SetsIsRunning()
        {
            var server = new ServerDedicatedServer();

            server.Start();

            Assert.True(server.IsRunning);
        }

        [Fact]
        public void Start_WhenAlreadyRunning_Throws()
        {
            var server = new ServerDedicatedServer();
            server.Start();

            Assert.Throws<InvalidOperationException>(() => server.Start());
        }

        [Fact]
        public void Stop_WhenNotRunning_Throws()
        {
            var server = new ServerDedicatedServer();

            Assert.Throws<InvalidOperationException>(() => server.Stop());
        }

        [Fact]
        public void Stop_DisconnectsEveryPlayer_AndRaisesPlayerLeft()
        {
            var server = new ServerDedicatedServer();
            server.Start();
            server.Join("Alice");
            server.Join("Bob");
            var left = new List<string>();
            server.PlayerLeft += username =>
            {
                Assert.False(server.IsRunning);
                left.Add(username);
            };

            server.Stop();

            Assert.False(server.IsRunning);
            Assert.Empty(server.Players);
            Assert.Equal(new[] { "Alice", "Bob" }, left);
        }

        [Fact]
        public void Join_BeforeStart_Throws()
        {
            var server = new ServerDedicatedServer();

            Assert.Throws<InvalidOperationException>(() => server.Join("Alice"));
        }

        [Fact]
        public void Join_EmptyUsername_Throws()
        {
            var server = new ServerDedicatedServer();
            server.Start();

            Assert.Throws<ArgumentException>(() => server.Join("   "));
        }

        [Fact]
        public void Join_AddsPlayer_InJoinOrder_AndRaisesPlayerJoined()
        {
            var server = new ServerDedicatedServer();
            server.Start();
            var joined = new List<string>();
            server.PlayerJoined += joined.Add;

            server.Join("Alice");
            server.Join("Bob");

            Assert.Equal(2, server.PlayerCount);
            Assert.Equal(new[] { "Alice", "Bob" }, server.Players);
            Assert.Equal(new[] { "Alice", "Bob" }, joined);
            Assert.True(server.IsOnline("Alice"));
            Assert.False(server.IsOnline("Carol"));
        }

        [Fact]
        public void Join_DuplicateUsername_Throws()
        {
            var server = new ServerDedicatedServer();
            server.Start();
            server.Join("Alice");

            Assert.Throws<InvalidOperationException>(() => server.Join("Alice"));
            Assert.Single(server.Players);
        }

        [Fact]
        public void Join_WhenServerIsFull_Throws()
        {
            var server = new ServerDedicatedServer(maxPlayers: 2);
            server.Start();
            server.Join("Alice");
            server.Join("Bob");

            Assert.Throws<InvalidOperationException>(() => server.Join("Carol"));
            Assert.Equal(2, server.PlayerCount);
        }

        [Fact]
        public void Constructor_MaxPlayersBelowOne_Throws()
        {
            Assert.Throws<ArgumentOutOfRangeException>(() => new ServerDedicatedServer(0));
        }

        [Fact]
        public void Constructor_DefaultsMaxPlayersAndCreatesOwnInstanceService()
        {
            var server = new ServerDedicatedServer();

            Assert.Equal(ServerDedicatedServer.DefaultMaxPlayers, server.MaxPlayers);
            Assert.NotNull(server.Instances);
        }

        [Fact]
        public void Constructor_UsesInjectedInstanceService()
        {
            var instances = new InstanceService();

            var server = new ServerDedicatedServer(instances: instances);

            Assert.Same(instances, server.Instances);
        }

        [Fact]
        public void Instances_IsAuthoritativeTreeOwnedByServer()
        {
            var server = new ServerDedicatedServer();

            var part = server.Instances.Create("Baseplate", "Part", "Workspace");

            Assert.Equal("Workspace", part.RootContainer);
        }

        [Fact]
        public void Leave_RemovesPlayer_AndRaisesPlayerLeft()
        {
            var server = new ServerDedicatedServer();
            server.Start();
            server.Join("Alice");
            server.Join("Bob");
            var left = new List<string>();
            server.PlayerLeft += left.Add;

            server.Leave("Alice");

            Assert.False(server.IsOnline("Alice"));
            Assert.Equal(new[] { "Alice" }, left);
            Assert.Equal("Bob", Assert.Single(server.Players));
        }

        [Fact]
        public void Leave_UnknownPlayer_Throws()
        {
            var server = new ServerDedicatedServer();
            server.Start();

            Assert.Throws<InvalidOperationException>(() => server.Leave("Ghost"));
        }

        [Fact]
        public void Rejoin_AfterLeave_IsAllowed()
        {
            var server = new ServerDedicatedServer(maxPlayers: 1);
            server.Start();
            server.Join("Alice");

            server.Leave("Alice");
            server.Join("Alice");

            Assert.True(server.IsOnline("Alice"));
            Assert.Equal("Alice", Assert.Single(server.Players));
        }
    }
}