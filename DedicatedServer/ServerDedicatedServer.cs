using System;
using System.Collections.Generic;
using Noobietoria.Studio.Core;

namespace Noobietoria.DedicatedServer
{
    /// <summary>
    /// The dedicated software server that hosts a Noobietoria place for every
    /// connected client.
    ///
    /// It owns the authoritative Instance tree through <see cref="InstanceService"/>
    /// (the same tree ServerScripts read and write), controls the server
    /// lifecycle via Start/Stop, and tracks which players are connected.
    ///
    /// Clients never run this class — they only ever see what this server
    /// chooses to replicate to them, which is what keeps server-only logic
    /// (economy, anti-cheat, data) safe from the client.
    ///
    /// See: https://noobietoria.github.io/Docs/en/instance/#serverscript
    /// </summary>
    public class ServerDedicatedServer
    {
        /// <summary>
        /// Capacity used when maxPlayers is not supplied to the constructor.
        /// </summary>
        public const int DefaultMaxPlayers = 16;

        // Connected players in join order, plus a set for O(1) membership
        // checks — same dual-index approach InstanceService uses for
        // names/GUIDs.
        private readonly List<string> _players = new();
        private readonly HashSet<string> _playerSet = new(StringComparer.Ordinal);

        /// <summary>
        /// Raised once a player has successfully joined the server.
        /// </summary>
        public event Action<string>? PlayerJoined;

        /// <summary>
        /// Raised once a player has disconnected, either through Leave or
        /// because the server was stopped.
        /// </summary>
        public event Action<string>? PlayerLeft;

        /// <summary>
        /// Creates a dedicated server.
        /// </summary>
        /// <param name="maxPlayers">Maximum concurrent players; must be at least 1.</param>
        /// <param name="instances">
        /// Authoritative Instance tree to host. Defaults to a fresh
        /// <see cref="InstanceService"/> when omitted.
        /// </param>
        public ServerDedicatedServer(int maxPlayers = DefaultMaxPlayers, InstanceService? instances = null)
        {
            if (maxPlayers < 1)
                throw new ArgumentOutOfRangeException(
                    nameof(maxPlayers), maxPlayers, "maxPlayers must be at least 1.");

            MaxPlayers = maxPlayers;
            Instances = instances ?? new InstanceService();
        }

        /// <summary>
        /// Authoritative Instance tree hosted by this server.
        /// </summary>
        public InstanceService Instances { get; }

        /// <summary>
        /// Maximum number of players that may be connected at once.
        /// </summary>
        public int MaxPlayers { get; }

        /// <summary>
        /// True between Start() and Stop().
        /// </summary>
        public bool IsRunning { get; private set; }

        /// <summary>
        /// Number of players currently connected.
        /// </summary>
        public int PlayerCount => _players.Count;

        /// <summary>
        /// Usernames currently connected, in join order.
        /// </summary>
        public IReadOnlyList<string> Players => _players;

        /// <summary>
        /// Starts the server so players can join. Throws if already running.
        /// </summary>
        public void Start()
        {
            if (IsRunning)
                throw new InvalidOperationException("Server is already running.");

            IsRunning = true;
        }

        /// <summary>
        /// Stops the server and disconnects every connected player, raising
        /// <see cref="PlayerLeft"/> for each of them. Handlers observe
        /// IsRunning == false. Throws if the server is not running.
        /// </summary>
        public void Stop()
        {
            if (!IsRunning)
                throw new InvalidOperationException("Server is not running.");

            IsRunning = false;
            foreach (var username in _players.ToArray())
                Leave(username);
        }

        /// <summary>
        /// Connects a player to the server. The server must be running, the
        /// name must be non-empty, unique and within capacity.
        /// </summary>
        public void Join(string username)
        {
            if (!IsRunning)
                throw new InvalidOperationException("Server is not running.");
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (_playerSet.Contains(username))
                throw new InvalidOperationException($"Player '{username}' is already connected.");
            if (_players.Count >= MaxPlayers)
                throw new InvalidOperationException(
                    $"Server is full ({MaxPlayers} players maximum).");

            _playerSet.Add(username);
            _players.Add(username);
            PlayerJoined?.Invoke(username);
        }

        /// <summary>
        /// Disconnects a player, raising <see cref="PlayerLeft"/>. Throws if
        /// no player with that username is connected.
        /// </summary>
        public void Leave(string username)
        {
            if (!_playerSet.Remove(username))
                throw new InvalidOperationException($"No player named '{username}' is connected.");

            _players.Remove(username);
            PlayerLeft?.Invoke(username);
        }

        /// <summary>
        /// True when the given username is currently connected.
        /// </summary>
        public bool IsOnline(string username) => _playerSet.Contains(username);
    }
}