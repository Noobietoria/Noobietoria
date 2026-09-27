using System;
using System.Collections.Generic;
using System.Linq;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// ChatService manages chat channels, chat bubbles, muting and word
    /// filter levels ("None", "Moderate", "Strict"). v0 stores the filter
    /// level per channel; the actual word list is applied by the platform
    /// filter later.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#chatservice
    /// </summary>
    public class ChatService
    {
        /// <summary>Supported filter levels.</summary>
        public static readonly IReadOnlyList<string> FilterLevels =
            new[] { "None", "Moderate", "Strict" };

        /// <summary>One chat message (OnMessage payload).</summary>
        public sealed record ChatMessage(string Username, string ChannelName, string Message, DateTime SentAt);

        /// <summary>One active chat bubble.</summary>
        public sealed record ChatBubble(string Username, string Message, double Duration);

        private readonly object _lock = new();
        private readonly Func<DateTime> _clock;
        private readonly Dictionary<string, (string FilterLevel, HashSet<string> Members, List<ChatMessage> Messages)>
            _channels = new(StringComparer.Ordinal);
        private readonly Dictionary<string, DateTime> _mutedUntil = new(StringComparer.Ordinal);
        private readonly Dictionary<string, ChatBubble> _bubbles = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action<ChatMessage>>> _messageCallbacks = new(StringComparer.Ordinal);

        public ChatService(Func<DateTime>? clock = null) => _clock = clock ?? (() => DateTime.UtcNow);

        /// <summary>ChatService.CreateChannel(ChannelName)</summary>
        public void CreateChannel(string channelName)
        {
            RequireChannel(channelName);
            lock (_lock)
            {
                if (!_channels.TryAdd(channelName, ("None", new HashSet<string>(StringComparer.Ordinal), new List<ChatMessage>())))
                    throw new InvalidOperationException($"Channel '{channelName}' already exists.");
            }
        }

        /// <summary>ChatService.DeleteChannel(ChannelName)</summary>
        public void DeleteChannel(string channelName)
        {
            RequireChannel(channelName);
            lock (_lock)
            {
                if (!_channels.Remove(channelName))
                    throw new InvalidOperationException($"No channel '{channelName}'.");
            }
        }

        /// <summary>ChatService.JoinChannel(Username, ChannelName)</summary>
        public void JoinChannel(string username, string channelName)
        {
            RequireUsername(username);
            RequireChannel(channelName);
            lock (_lock)
            {
                Channel(channelName).Members.Add(username);
            }
        }

        /// <summary>ChatService.LeaveChannel(Username, ChannelName)</summary>
        public void LeaveChannel(string username, string channelName)
        {
            RequireUsername(username);
            RequireChannel(channelName);
            lock (_lock)
            {
                Channel(channelName).Members.Remove(username);
            }
        }

        /// <summary>ChatService.SendMessage(Username, ChannelName, Message)</summary>
        public ChatMessage SendMessage(string username, string channelName, string message)
        {
            RequireUsername(username);
            RequireChannel(channelName);
            if (string.IsNullOrWhiteSpace(message))
                throw new ArgumentException("Message must not be empty.", nameof(message));

            ChatMessage chatMessage;
            List<Action<ChatMessage>>? callbacks;
            lock (_lock)
            {
                var channel = Channel(channelName);
                if (!channel.Members.Contains(username))
                    throw new InvalidOperationException($"'{username}' has not joined channel '{channelName}'.");
                if (_mutedUntil.TryGetValue(username, out var until) && _clock() < until)
                    throw new InvalidOperationException($"'{username}' is muted until {until:HH:mm:ss}.");

                chatMessage = new ChatMessage(username, channelName, message, _clock());
                channel.Messages.Add(chatMessage);
                _messageCallbacks.TryGetValue(channelName, out callbacks);
            }
            callbacks?.ForEach(cb => cb(chatMessage));
            return chatMessage;
        }

        /// <summary>ChatService.MutePlayer(Username, Duration) — Duration in seconds.</summary>
        public void MutePlayer(string username, double duration)
        {
            RequireUsername(username);
            if (duration < 0)
                throw new ArgumentException("Duration must not be negative.", nameof(duration));
            lock (_lock)
            {
                _mutedUntil[username] = _clock().AddSeconds(duration);
            }
        }

        /// <summary>ChatService.UnmutePlayer(Username)</summary>
        public void UnmutePlayer(string username)
        {
            RequireUsername(username);
            lock (_lock)
            {
                _mutedUntil.Remove(username);
            }
        }

        /// <summary>ChatService.SetFilter(ChannelName, FilterLevel)</summary>
        public void SetFilter(string channelName, string filterLevel)
        {
            RequireChannel(channelName);
            string? canonical = FilterLevels.FirstOrDefault(f =>
                string.Equals(f, filterLevel, StringComparison.OrdinalIgnoreCase));
            if (canonical == null)
                throw new ArgumentException(
                    $"FilterLevel '{filterLevel}' is not supported. Allowed: {string.Join(", ", FilterLevels)}.",
                    nameof(filterLevel));

            lock (_lock)
            {
                var channel = Channel(channelName);
                _channels[channelName] = (canonical, channel.Members, channel.Messages);
            }
        }

        /// <summary>ChatService.ShowBubble(Username, Message, Duration)</summary>
        public void ShowBubble(string username, string message, double duration = 3)
        {
            RequireUsername(username);
            if (string.IsNullOrWhiteSpace(message))
                throw new ArgumentException("Message must not be empty.", nameof(message));
            if (duration < 0)
                throw new ArgumentException("Duration must not be negative.", nameof(duration));
            lock (_lock)
            {
                _bubbles[username] = new ChatBubble(username, message, duration);
            }
        }

        /// <summary>ChatService.HideBubble(Username)</summary>
        public void HideBubble(string username)
        {
            RequireUsername(username);
            lock (_lock)
            {
                _bubbles.Remove(username);
            }
        }

        /// <summary>ChatService.OnMessage(ChannelName, Callback)</summary>
        public void OnMessage(string channelName, Action<ChatMessage> callback)
        {
            RequireChannel(channelName);
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                if (!_messageCallbacks.TryGetValue(channelName, out var list))
                    _messageCallbacks[channelName] = list = new List<Action<ChatMessage>>();
                list.Add(callback);
            }
        }

        /// <summary>Active bubble of a player (tests/tooling).</summary>
        public ChatBubble? GetBubble(string username)
        {
            lock (_lock)
            {
                return _bubbles.GetValueOrDefault(username);
            }
        }

        private (string FilterLevel, HashSet<string> Members, List<ChatMessage> Messages) Channel(string channelName)
        {
            return _channels.TryGetValue(channelName, out var channel)
                ? channel
                : throw new InvalidOperationException($"No channel '{channelName}'.");
        }

        private static void RequireChannel(string channelName)
        {
            if (string.IsNullOrWhiteSpace(channelName))
                throw new ArgumentException("ChannelName must not be empty.", nameof(channelName));
        }

        private static void RequireUsername(string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
        }
    }

    /// <summary>
    /// PartyService creates parties with a leader, MaxSize, invites, kicking
    /// and leader transfer. PartyIds are auto-generated ("PTY-nnn").
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#partyservice
    /// </summary>
    public class PartyService
    {
        /// <summary>A party snapshot (GetParty payload).</summary>
        public sealed record PartySnapshot(string PartyId, string Leader, int MaxSize, IReadOnlyList<string> Members);

        private sealed class Party
        {
            public string Leader = "";
            public int MaxSize;
            public List<string> Members = new();
            public HashSet<string> Invites = new(StringComparer.Ordinal);
        }

        private int _nextPartyId = 1;
        private readonly object _lock = new();
        private readonly Dictionary<string, Party> _parties = new(StringComparer.Ordinal);
        private readonly Dictionary<string, string> _partyOf = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action<string>>> _inviteCallbacks = new(StringComparer.Ordinal);

        /// <summary>PartyService.CreateParty(Leader, MaxSize) — returns the PartyId.</summary>
        public string CreateParty(string leader, int maxSize)
        {
            if (string.IsNullOrWhiteSpace(leader))
                throw new ArgumentException("Leader must not be empty.", nameof(leader));
            if (maxSize < 1)
                throw new ArgumentException("MaxSize must be at least 1.", nameof(maxSize));

            lock (_lock)
            {
                if (_partyOf.ContainsKey(leader))
                    throw new InvalidOperationException($"'{leader}' is already in a party.");

                string partyId = $"PTY-{_nextPartyId++:0000}";
                _parties[partyId] = new Party { Leader = leader, MaxSize = maxSize, Members = { leader } };
                _partyOf[leader] = partyId;
                return partyId;
            }
        }

        /// <summary>PartyService.DisbandParty(PartyId)</summary>
        public void DisbandParty(string partyId)
        {
            lock (_lock)
            {
                var party = RequireParty(partyId);
                foreach (string member in party.Members)
                    _partyOf.Remove(member);
                _parties.Remove(partyId);
            }
        }

        /// <summary>PartyService.Invite(PartyId, Username)</summary>
        public void Invite(string partyId, string username)
        {
            RequireUsername(username);
            List<Action<string>>? callbacks;
            lock (_lock)
            {
                var party = RequireParty(partyId);
                if (party.Members.Contains(username))
                    throw new InvalidOperationException($"'{username}' is already in party '{partyId}'.");
                if (!party.Invites.Add(username))
                    throw new InvalidOperationException($"'{username}' is already invited to '{partyId}'.");
                _inviteCallbacks.TryGetValue(username, out callbacks);
            }
            callbacks?.ForEach(cb => cb(partyId));
        }

        /// <summary>PartyService.AcceptInvite(PartyId, Username)</summary>
        public void AcceptInvite(string partyId, string username)
        {
            RequireUsername(username);
            lock (_lock)
            {
                var party = RequireParty(partyId);
                if (!party.Invites.Remove(username))
                    throw new InvalidOperationException($"'{username}' has no invite to '{partyId}'.");
                if (party.Members.Count >= party.MaxSize)
                    throw new InvalidOperationException($"Party '{partyId}' is full ({party.MaxSize}).");
                if (_partyOf.ContainsKey(username))
                    throw new InvalidOperationException($"'{username}' is already in a party.");
                party.Members.Add(username);
                _partyOf[username] = partyId;
            }
        }

        /// <summary>PartyService.DeclineInvite(PartyId, Username)</summary>
        public void DeclineInvite(string partyId, string username)
        {
            RequireUsername(username);
            lock (_lock)
            {
                var party = RequireParty(partyId);
                if (!party.Invites.Remove(username))
                    throw new InvalidOperationException($"'{username}' has no invite to '{partyId}'.");
            }
        }

        /// <summary>PartyService.Kick(PartyId, Username) — the leader cannot be kicked.</summary>
        public void Kick(string partyId, string username)
        {
            RequireUsername(username);
            lock (_lock)
            {
                var party = RequireParty(partyId);
                if (username == party.Leader)
                    throw new InvalidOperationException("The party leader cannot be kicked.");
                if (!party.Members.Remove(username))
                    throw new InvalidOperationException($"'{username}' is not in party '{partyId}'.");
                _partyOf.Remove(username);
            }
        }

        /// <summary>PartyService.Leave(PartyId, Username) — the leader must transfer leadership or disband.</summary>
        public void Leave(string partyId, string username)
        {
            RequireUsername(username);
            lock (_lock)
            {
                var party = RequireParty(partyId);
                if (!party.Members.Contains(username))
                    throw new InvalidOperationException($"'{username}' is not in party '{partyId}'.");
                if (username == party.Leader && party.Members.Count > 1)
                    throw new InvalidOperationException(
                        "The leader must TransferLeader before leaving (or disband the party).");

                party.Members.Remove(username);
                _partyOf.Remove(username);
                if (party.Members.Count == 0)
                    _parties.Remove(partyId);
            }
        }

        /// <summary>PartyService.GetParty(PartyId)</summary>
        public PartySnapshot? GetParty(string partyId)
        {
            lock (_lock)
            {
                return _parties.GetValueOrDefault(partyId) is { } party
                    ? new PartySnapshot(partyId, party.Leader, party.MaxSize, party.Members.ToArray())
                    : null;
            }
        }

        /// <summary>PartyService.GetPartyOf(Username)</summary>
        public PartySnapshot? GetPartyOf(string username)
        {
            RequireUsername(username);
            lock (_lock)
            {
                return _partyOf.TryGetValue(username, out var partyId) ? GetParty(partyId) : null;
            }
        }

        /// <summary>PartyService.TransferLeader(PartyId, Username)</summary>
        public void TransferLeader(string partyId, string username)
        {
            RequireUsername(username);
            lock (_lock)
            {
                var party = RequireParty(partyId);
                if (!party.Members.Contains(username))
                    throw new InvalidOperationException($"'{username}' is not in party '{partyId}'.");
                party.Leader = username;
            }
        }

        /// <summary>PartyService.OnInvite(Username, Callback) — receives the PartyId.</summary>
        public void OnInvite(string username, Action<string> callback)
        {
            RequireUsername(username);
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                if (!_inviteCallbacks.TryGetValue(username, out var list))
                    _inviteCallbacks[username] = list = new List<Action<string>>();
                list.Add(callback);
            }
        }

        private Party RequireParty(string partyId)
        {
            if (string.IsNullOrWhiteSpace(partyId))
                throw new ArgumentException("PartyId must not be empty.", nameof(partyId));
            return _parties.GetValueOrDefault(partyId)
                ?? throw new InvalidOperationException($"No party '{partyId}'.");
        }

        private static void RequireUsername(string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
        }
    }

    /// <summary>
    /// DialogueService handles NPC dialogue chains: nodes with text, choices
    /// and optional callbacks. Choosing choice N (1-based) jumps to node N;
    /// a choice beyond the node count ends the dialogue.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#dialogueservice
    /// </summary>
    public class DialogueService
    {
        /// <summary>One dialogue node: text, choices, optional callback.</summary>
        public sealed record DialogueNode(string Text, IReadOnlyList<string> Choices, Action? Callback = null);

        /// <summary>A registered dialogue chain.</summary>
        public sealed record DialogueSpec(string DialogueId, string NpcName, IReadOnlyList<DialogueNode> Nodes);

        /// <summary>The active dialogue state of a player (GetCurrent payload).</summary>
        public sealed record DialogueCurrent(string DialogueId, string NpcName, int NodeIndex, string Text,
            IReadOnlyList<string> Choices);

        private sealed class Session
        {
            public string DialogueId = "";
            public int NodeIndex;
        }

        private readonly object _lock = new();
        private readonly Dictionary<string, DialogueSpec> _dialogues = new(StringComparer.Ordinal);
        private readonly Dictionary<string, Session> _sessions = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action<string>>> _startCallbacks = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action<string>>> _endCallbacks = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action<string, int, int>>> _choiceCallbacks = new(StringComparer.Ordinal);

        /// <summary>DialogueService.Register(DialogueId, NpcName, Nodes)</summary>
        public void Register(string dialogueId, string npcName, IReadOnlyList<DialogueNode> nodes)
        {
            RequireId(dialogueId);
            if (string.IsNullOrWhiteSpace(npcName))
                throw new ArgumentException("NpcName must not be empty.", nameof(npcName));
            if (nodes == null || nodes.Count == 0)
                throw new ArgumentException("A dialogue needs at least one node.", nameof(nodes));

            lock (_lock)
            {
                if (!_dialogues.TryAdd(dialogueId, new DialogueSpec(dialogueId, npcName, nodes.ToArray())))
                    throw new InvalidOperationException($"Dialogue '{dialogueId}' is already registered.");
            }
        }

        /// <summary>DialogueService.Unregister(DialogueId)</summary>
        public void Unregister(string dialogueId)
        {
            RequireId(dialogueId);
            lock (_lock)
            {
                if (!_dialogues.Remove(dialogueId))
                    throw new InvalidOperationException($"No dialogue '{dialogueId}'.");
            }
        }

        /// <summary>DialogueService.Start(Username, DialogueId)</summary>
        public void Start(string username, string dialogueId)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            var spec = RequireDialogue(dialogueId);

            List<Action<string>>? callbacks;
            lock (_lock)
            {
                if (_sessions.ContainsKey(username))
                    throw new InvalidOperationException($"'{username}' is already in a dialogue — End it first.");
                _sessions[username] = new Session { DialogueId = dialogueId, NodeIndex = 0 };
                _startCallbacks.TryGetValue(spec.NpcName, out callbacks);
            }
            callbacks?.ForEach(cb => cb(spec.NpcName));
        }

        /// <summary>DialogueService.End(Username)</summary>
        public void End(string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));

            List<Action<string>>? callbacks;
            string npcName;
            lock (_lock)
            {
                if (!_sessions.Remove(username, out var session))
                    throw new InvalidOperationException($"'{username}' is not in a dialogue.");
                npcName = _dialogues.GetValueOrDefault(session.DialogueId)!.NpcName;
                _endCallbacks.TryGetValue(npcName, out callbacks);
            }
            callbacks?.ForEach(cb => cb(npcName));
        }

        /// <summary>DialogueService.GetCurrent(Username) — null when not in a dialogue.</summary>
        public DialogueCurrent? GetCurrent(string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            lock (_lock)
            {
                if (!_sessions.TryGetValue(username, out var session))
                    return null;
                var spec = _dialogues.GetValueOrDefault(session.DialogueId)!;
                var node = spec.Nodes[session.NodeIndex];
                return new DialogueCurrent(spec.DialogueId, spec.NpcName, session.NodeIndex + 1, node.Text, node.Choices);
            }
        }

        /// <summary>
        /// DialogueService.Choose(Username, ChoiceIndex) — 1-based; choice N
        /// navigates to node N (0-based index N) or ends the dialogue when
        /// node N does not exist.
        /// </summary>
        public void Choose(string username, int choiceIndex)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (choiceIndex < 1)
                throw new ArgumentException("ChoiceIndex is 1-based and must be >= 1.", nameof(choiceIndex));

            Action<string, int, int>[]? callbacks;
            lock (_lock)
            {
                if (!_sessions.TryGetValue(username, out var session))
                    throw new InvalidOperationException($"'{username}' is not in a dialogue.");
                var spec = _dialogues.GetValueOrDefault(session.DialogueId)!;
                var node = spec.Nodes[session.NodeIndex];
                if (node.Choices.Count == 0)
                    throw new InvalidOperationException($"Node {session.NodeIndex + 1} has no choices.");
                if (choiceIndex > node.Choices.Count)
                    throw new ArgumentException(
                        $"Node has {node.Choices.Count} choice(s); got choice {choiceIndex}.", nameof(choiceIndex));

                callbacks = _choiceCallbacks.TryGetValue(spec.DialogueId, out var list)
                    ? list.ToArray()
                    : Array.Empty<Action<string, int, int>>();

                if (choiceIndex < spec.Nodes.Count)
                {
                    session.NodeIndex = choiceIndex;
                    spec.Nodes[session.NodeIndex].Callback?.Invoke();
                }
                else
                {
                    _sessions.Remove(username);
                }
            }
            foreach (var callback in callbacks)
                callback(username, choiceIndex, choiceIndex);
        }

        /// <summary>DialogueService.OnStart(NpcName, Callback) — receives the NpcName.</summary>
        public void OnStart(string npcName, Action<string> callback)
        {
            AddCallback(_startCallbacks, npcName, callback);
        }

        /// <summary>DialogueService.OnEnd(NpcName, Callback) — receives the NpcName.</summary>
        public void OnEnd(string npcName, Action<string> callback)
        {
            AddCallback(_endCallbacks, npcName, callback);
        }

        /// <summary>DialogueService.OnChoice(DialogueId, Callback) — receives (username, chosen node, choice index).</summary>
        public void OnChoice(string dialogueId, Action<string, int, int> callback)
        {
            AddCallback(_choiceCallbacks, dialogueId, callback);
        }

        private DialogueSpec RequireDialogue(string dialogueId)
        {
            RequireId(dialogueId);
            return _dialogues.GetValueOrDefault(dialogueId)
                ?? throw new InvalidOperationException($"No dialogue '{dialogueId}'.");
        }

        private static void RequireId(string dialogueId)
        {
            if (string.IsNullOrWhiteSpace(dialogueId))
                throw new ArgumentException("DialogueId must not be empty.", nameof(dialogueId));
        }

        private static void AddCallback<K1, K2>(Dictionary<K1, List<K2>> map, K1 key, K2 callback)
            where K1 : notnull
        {
            if (string.IsNullOrWhiteSpace(key?.ToString()))
                throw new ArgumentException("Key must not be empty.");
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (map)
            {
                if (!map.TryGetValue(key, out var list))
                    map[key] = list = new List<K2>();
                list.Add(callback);
            }
        }
    }

    /// <summary>
    /// MatchmakingService queues players and matches them into rooms. When a
    /// queue reaches MaxPlayers a room is created automatically and
    /// OnMatch fires.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#matchmakingservice
    /// </summary>
    public class MatchmakingService
    {
        /// <summary>A queue snapshot (GetQueue payload).</summary>
        public sealed record QueueSnapshot(string QueueName, int MaxPlayers, IReadOnlyList<string> Queued);

        /// <summary>A room snapshot (GetRoom / GetRooms payload).</summary>
        public sealed record RoomSnapshot(string RoomId, string QueueName, IReadOnlyList<string> Players);

        private int _nextRoomId = 1;
        private readonly object _lock = new();
        private readonly Dictionary<string, (int MaxPlayers, List<string> Queue)> _queues =
            new(StringComparer.Ordinal);
        private readonly Dictionary<string, RoomSnapshot> _rooms = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action<QueueSnapshot, RoomSnapshot>>> _matchCallbacks =
            new(StringComparer.Ordinal);

        /// <summary>MatchmakingService.CreateQueue(QueueName, MaxPlayers)</summary>
        public void CreateQueue(string queueName, int maxPlayers)
        {
            RequireQueueName(queueName);
            if (maxPlayers < 1)
                throw new ArgumentException("MaxPlayers must be at least 1.", nameof(maxPlayers));
            lock (_lock)
            {
                if (!_queues.TryAdd(queueName, (maxPlayers, new List<string>())))
                    throw new InvalidOperationException($"Queue '{queueName}' already exists.");
            }
        }

        /// <summary>MatchmakingService.DeleteQueue(QueueName)</summary>
        public void DeleteQueue(string queueName)
        {
            RequireQueueName(queueName);
            lock (_lock)
            {
                if (!_queues.Remove(queueName))
                    throw new InvalidOperationException($"No queue '{queueName}'.");
                foreach (string roomId in _rooms.Where(kv => kv.Value.QueueName == queueName).Select(kv => kv.Key).ToArray())
                    _rooms.Remove(roomId);
            }
        }

        /// <summary>MatchmakingService.Join(Username, QueueName)</summary>
        public void Join(string username, string queueName)
        {
            RequireUsername(username);
            RequireQueueName(queueName);
            RoomSnapshot? room = null;
            List<Action<QueueSnapshot, RoomSnapshot>>? callbacks;
            lock (_lock)
            {
                if (!_queues.TryGetValue(queueName, out var queue))
                    throw new InvalidOperationException($"No queue '{queueName}'.");
                if (queue.Queue.Contains(username))
                    throw new InvalidOperationException($"'{username}' is already queued for '{queueName}'.");

                queue.Queue.Add(username);
                if (queue.Queue.Count >= queue.MaxPlayers)
                {
                    string roomId = $"ROOM-{_nextRoomId++:0000}";
                    room = new RoomSnapshot(roomId, queueName, queue.Queue.ToArray());
                    _rooms[roomId] = room;
                    queue.Queue.Clear();
                    _matchCallbacks.TryGetValue(queueName, out callbacks);
                }
                else
                {
                    callbacks = null;
                }
            }
            if (room != null)
                callbacks?.ForEach(cb => cb(GetQueue(queueName)!, room));
        }

        /// <summary>MatchmakingService.Leave(Username, QueueName)</summary>
        public void Leave(string username, string queueName)
        {
            RequireUsername(username);
            RequireQueueName(queueName);
            lock (_lock)
            {
                if (!_queues.TryGetValue(queueName, out var queue) || !queue.Queue.Remove(username))
                    throw new InvalidOperationException($"'{username}' is not queued for '{queueName}'.");
            }
        }

        /// <summary>MatchmakingService.GetQueue(QueueName)</summary>
        public QueueSnapshot? GetQueue(string queueName)
        {
            RequireQueueName(queueName);
            lock (_lock)
            {
                return _queues.TryGetValue(queueName, out var queue)
                    ? new QueueSnapshot(queueName, queue.MaxPlayers, queue.Queue.ToArray())
                    : null;
            }
        }

        /// <summary>MatchmakingService.CreateRoom(RoomId, QueueName)</summary>
        public void CreateRoom(string roomId, string queueName)
        {
            if (string.IsNullOrWhiteSpace(roomId))
                throw new ArgumentException("RoomId must not be empty.", nameof(roomId));
            RequireQueueName(queueName);
            lock (_lock)
            {
                if (!_queues.ContainsKey(queueName))
                    throw new InvalidOperationException($"No queue '{queueName}'.");
                if (!_rooms.TryAdd(roomId, new RoomSnapshot(roomId, queueName, Array.Empty<string>())))
                    throw new InvalidOperationException($"Room '{roomId}' already exists.");
            }
        }

        /// <summary>MatchmakingService.CloseRoom(RoomId)</summary>
        public void CloseRoom(string roomId)
        {
            if (string.IsNullOrWhiteSpace(roomId))
                throw new ArgumentException("RoomId must not be empty.", nameof(roomId));
            lock (_lock)
            {
                if (!_rooms.Remove(roomId))
                    throw new InvalidOperationException($"No room '{roomId}'.");
            }
        }

        /// <summary>MatchmakingService.GetRoom(RoomId)</summary>
        public RoomSnapshot? GetRoom(string roomId)
        {
            if (string.IsNullOrWhiteSpace(roomId))
                throw new ArgumentException("RoomId must not be empty.", nameof(roomId));
            lock (_lock)
            {
                return _rooms.GetValueOrDefault(roomId);
            }
        }

        /// <summary>MatchmakingService.GetRooms(QueueName)</summary>
        public IReadOnlyList<RoomSnapshot> GetRooms(string queueName)
        {
            RequireQueueName(queueName);
            lock (_lock)
            {
                return _rooms.Values.Where(r => r.QueueName == queueName).ToArray();
            }
        }

        /// <summary>MatchmakingService.OnMatch(QueueName, Callback) — receives (queue, room).</summary>
        public void OnMatch(string queueName, Action<QueueSnapshot, RoomSnapshot> callback)
        {
            RequireQueueName(queueName);
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                if (!_matchCallbacks.TryGetValue(queueName, out var list))
                    _matchCallbacks[queueName] = list = new List<Action<QueueSnapshot, RoomSnapshot>>();
                list.Add(callback);
            }
        }

        private static void RequireUsername(string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
        }

        private static void RequireQueueName(string queueName)
        {
            if (string.IsNullOrWhiteSpace(queueName))
                throw new ArgumentException("QueueName must not be empty.", nameof(queueName));
        }
    }

    /// <summary>
    /// BanService (also known as PunishService) kicks, warns and bans rule-
    /// breaking players; Duration -1 means a permanent ban. All actions are
    /// recorded in the player's punishment history.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#banservice
    /// </summary>
    public class BanService
    {
        /// <summary>Punishment kinds recorded in history.</summary>
        public static readonly IReadOnlyList<string> Kinds = new[] { "Kick", "Ban", "Unban", "Warn" };

        /// <summary>One history entry (GetBanHistory payload).</summary>
        public sealed record PunishmentRecord(string Username, string Kind, string Reason, DateTime At);

        /// <summary>Active ban info (GetBanInfo payload).</summary>
        public sealed record BanInfo(string Username, string Reason, DateTime? Until, bool Permanent);

        private readonly object _lock = new();
        private readonly Func<DateTime> _clock;
        private readonly Dictionary<string, (string Reason, DateTime? Until)> _bans = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<string>> _warnings = new(StringComparer.Ordinal);
        private readonly List<PunishmentRecord> _history = new();

        public BanService(Func<DateTime>? clock = null) => _clock = clock ?? (() => DateTime.UtcNow);

        /// <summary>BanService.Kick(Username, Reason)</summary>
        public void Kick(string username, string reason)
        {
            Require(username, reason);
            lock (_lock)
            {
                _history.Add(new PunishmentRecord(username, "Kick", reason, _clock()));
            }
        }

        /// <summary>BanService.Ban(Username, Reason, Duration) — Duration -1 is permanent.</summary>
        public void Ban(string username, string reason, double duration)
        {
            Require(username, reason);
            if (duration != -1 && duration <= 0)
                throw new ArgumentException("Duration must be -1 (permanent) or a positive number of seconds.",
                    nameof(duration));

            lock (_lock)
            {
                bool permanent = duration == -1;
                _bans[username] = (reason, permanent ? null : _clock().AddSeconds(duration));
                _history.Add(new PunishmentRecord(username, "Ban", reason, _clock()));
            }
        }

        /// <summary>BanService.Unban(Username)</summary>
        public void Unban(string username)
        {
            Require(username, "Unban");
            lock (_lock)
            {
                if (!_bans.Remove(username))
                    throw new InvalidOperationException($"'{username}' is not banned.");
                _history.Add(new PunishmentRecord(username, "Unban", string.Empty, _clock()));
            }
        }

        /// <summary>BanService.IsBanned(Username) — expired bans are lazily lifted.</summary>
        public bool IsBanned(string username)
        {
            Require(username, "IsBanned");
            lock (_lock)
            {
                return ActiveBan(username) != null;
            }
        }

        /// <summary>BanService.GetBanInfo(Username) — null when not banned.</summary>
        public BanInfo? GetBanInfo(string username)
        {
            Require(username, "GetBanInfo");
            lock (_lock)
            {
                var ban = ActiveBan(username);
                return ban.HasValue
                    ? new BanInfo(username, ban.Value.Reason, ban.Value.Until, ban.Value.Until == null)
                    : null;
            }
        }

        /// <summary>BanService.GetBanHistory(Username, Limit)</summary>
        public IReadOnlyList<PunishmentRecord> GetBanHistory(string username, int limit = 50)
        {
            Require(username, "GetBanHistory");
            MarketplaceService.RequireLimit(limit);
            lock (_lock)
            {
                return _history.Where(h => h.Username == username).TakeLast(limit).ToArray();
            }
        }

        /// <summary>BanService.Warn(Username, Reason)</summary>
        public void Warn(string username, string reason)
        {
            Require(username, reason);
            lock (_lock)
            {
                if (!_warnings.TryGetValue(username, out var list))
                    _warnings[username] = list = new List<string>();
                list.Add(reason);
                _history.Add(new PunishmentRecord(username, "Warn", reason, _clock()));
            }
        }

        /// <summary>BanService.GetWarnings(Username)</summary>
        public IReadOnlyList<string> GetWarnings(string username)
        {
            Require(username, "GetWarnings");
            lock (_lock)
            {
                return _warnings.GetValueOrDefault(username)?.ToArray() ?? Array.Empty<string>();
            }
        }

        /// <summary>BanService.ClearWarnings(Username)</summary>
        public void ClearWarnings(string username)
        {
            Require(username, "ClearWarnings");
            lock (_lock)
            {
                _warnings.Remove(username);
            }
        }

        private (string Reason, DateTime? Until)? ActiveBan(string username)
        {
            if (!_bans.TryGetValue(username, out var ban))
                return null;
            if (ban.Until.HasValue && _clock() >= ban.Until.Value)
            {
                _bans.Remove(username);
                return null;
            }
            return ban;
        }

        private static void Require(string username, string reason)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (reason == null)
                throw new ArgumentNullException(nameof(reason));
        }
    }
}
