using System;
using System.Linq;
using Noobietoria.Studio.Core;
using Xunit;

namespace Noobietoria.Studio.Tests
{
    public class ChatServiceTests
    {
        [Fact]
        public void Channels_Join_Send_Mute_Filter()
        {
            var now = new DateTime(2026, 1, 1, 10, 0, 0, DateTimeKind.Utc);
            var service = new ChatService(() => now);
            var messages = new System.Collections.Generic.List<ChatService.ChatMessage>();
            service.OnMessage("Global", m => messages.Add(m));

            service.CreateChannel("Global");
            Assert.Throws<InvalidOperationException>(() => service.CreateChannel("Global"));
            service.JoinChannel("Alice", "Global");
            service.JoinChannel("Bob", "Global");

            service.SendMessage("Alice", "Global", "hello");
            Assert.Single(messages);

            service.MutePlayer("Alice", 60);
            Assert.Throws<InvalidOperationException>(() => service.SendMessage("Alice", "Global", "spam"));
            service.UnmutePlayer("Alice");
            service.SendMessage("Alice", "Global", "sorry");

            service.SetFilter("Global", "Strict");
            Assert.Throws<ArgumentException>(() => service.SetFilter("Global", "Nuclear"));

            service.ShowBubble("Alice", "hi there", 2);
            Assert.Equal("hi there", service.GetBubble("Alice")!.Message);
            service.HideBubble("Alice");
            Assert.Null(service.GetBubble("Alice"));

            service.LeaveChannel("Alice", "Global");
            Assert.Throws<InvalidOperationException>(() => service.SendMessage("Alice", "Global", "echo"));

            service.DeleteChannel("Global");
            Assert.Throws<InvalidOperationException>(() => service.SendMessage("Bob", "Global", "anyone?"));
        }
    }

    public class PartyServiceTests
    {
        [Fact]
        public void Invite_Accept_Kick_Transfer()
        {
            var service = new PartyService();
            var invites = new System.Collections.Generic.List<string>();
            service.OnInvite("Bob", id => invites.Add(id));

            string partyId = service.CreateParty("Alice", 3);
            Assert.Equal("Alice", service.GetParty(partyId)!.Leader);

            service.Invite(partyId, "Bob");
            Assert.Equal(partyId, invites.Single());
            service.AcceptInvite(partyId, "Bob");
            Assert.Equal(2, service.GetParty(partyId)!.Members.Count);

            service.Invite(partyId, "Carol");
            Assert.Throws<InvalidOperationException>(() =>
                service.Kick(partyId, "Carol")); // invited but never accepted — not a member
            Assert.Throws<InvalidOperationException>(() => service.Kick(partyId, "Alice")); // leader protected

            service.DeclineInvite(partyId, "Carol");
            Assert.Throws<InvalidOperationException>(() => service.AcceptInvite(partyId, "Carol"));

            service.TransferLeader(partyId, "Bob");
            Assert.Equal("Bob", service.GetParty(partyId)!.Leader);

            Assert.Throws<InvalidOperationException>(() => service.Leave(partyId, "Bob")); // leader must transfer first
            service.TransferLeader(partyId, "Alice");
            service.Leave(partyId, "Bob");
            service.Leave(partyId, "Alice");
            Assert.Null(service.GetParty(partyId)); // last member left -> disbanded
        }

        [Fact]
        public void GetPartyOf_AndMaxSize()
        {
            var service = new PartyService();
            string partyId = service.CreateParty("Alice", 2);
            service.Invite(partyId, "Bob");
            service.AcceptInvite(partyId, "Bob");

            Assert.Equal("Alice", service.GetPartyOf("Alice")!.Leader);
            service.Invite(partyId, "Carol");
            Assert.Throws<InvalidOperationException>(() => service.AcceptInvite(partyId, "Carol"));
            Assert.Equal(2, service.GetParty(partyId)!.Members.Count);
        }
    }

    public class DialogueServiceTests
    {
        [Fact]
        public void Dialogue_Chain_Choices_AndCallbacks()
        {
            var service = new DialogueService();
            int started = 0, ended = 0, choices = 0;
            service.OnStart("ShopKeeper", _ => started++);
            service.OnEnd("ShopKeeper", _ => ended++);
            service.OnChoice("shop-dialog", (_, _, _) => choices++);

            service.Register("shop-dialog", "ShopKeeper", new[]
            {
                new DialogueService.DialogueNode("Welcome!", new[] { "Show wares", "Bye" }),
                new DialogueService.DialogueNode("Take a look.", Array.Empty<string>()),
            });

            service.Start("Alice", "shop-dialog");
            Assert.Equal(1, started);
            Assert.Equal("Welcome!", service.GetCurrent("Alice")!.Text);
            Assert.Throws<InvalidOperationException>(() => service.Start("Alice", "shop-dialog"));

            service.Choose("Alice", 1); // jumps to node 1
            Assert.Equal("Take a look.", service.GetCurrent("Alice")!.Text);

            Assert.Throws<InvalidOperationException>(() => service.Choose("Alice", 1));
            Assert.Equal(1, choices);

            service.End("Alice");
            Assert.Equal(1, ended);
            Assert.Null(service.GetCurrent("Alice"));

            Assert.Throws<ArgumentException>(() => service.Choose("Alice", 0));
            Assert.Throws<InvalidOperationException>(() => service.Choose("Alice", 1));
        }
    }

    public class MatchmakingServiceTests
    {
        [Fact]
        public void Queue_AutoMatch_Rooms()
        {
            var service = new MatchmakingService();
            (MatchmakingService.QueueSnapshot, MatchmakingService.RoomSnapshot)? matched = null;
            service.OnMatch("1v1", (queue, room) => matched = (queue, room));

            service.CreateQueue("1v1", 2);
            service.Join("Alice", "1v1");
            Assert.Null(matched);
            Assert.Equal(new[] { "Alice" }, service.GetQueue("1v1")!.Queued);

            service.Join("Bob", "1v1");
            Assert.NotNull(matched); // queue filled -> room created
            var room = matched!.Value.Item2;
            Assert.Equal(new[] { "Alice", "Bob" }, room.Players);

            Assert.Empty(service.GetQueue("1v1")!.Queued);
            Assert.Single(service.GetRooms("1v1"));
            service.CloseRoom(room.RoomId);
            Assert.Null(service.GetRoom(room.RoomId));

            Assert.Throws<InvalidOperationException>(() => service.Join("Alice", "1v1-ghost"));
            Assert.Throws<InvalidOperationException>(() => service.Leave("Alice", "1v1"));
        }
    }

    public class BanServiceTests
    {
        [Fact]
        public void Kick_Warn_Ban_Expire_History()
        {
            var now = new DateTime(2026, 1, 1, 10, 0, 0, DateTimeKind.Utc);
            var service = new BanService(() => now);

            service.Warn("Bob", "be nice");
            service.Warn("Bob", "seriously");
            Assert.Equal(2, service.GetWarnings("Bob").Count);
            service.ClearWarnings("Bob");
            Assert.Empty(service.GetWarnings("Bob"));

            service.Kick("Bob", "one more chance");
            service.Ban("Bob", "griefing", 60);
            Assert.True(service.IsBanned("Bob"));
            Assert.False(service.GetBanInfo("Bob")!.Permanent);
            Assert.Equal("griefing", service.GetBanInfo("Bob")!.Reason);

            now = now.AddSeconds(61);
            Assert.False(service.IsBanned("Bob")); // expired
            Assert.Null(service.GetBanInfo("Bob"));

            service.Ban("Bob", "repeat offender", -1);
            Assert.True(service.GetBanInfo("Bob")!.Permanent);

            Assert.Equal(5, service.GetBanHistory("Bob").Count());
            Assert.Throws<InvalidOperationException>(() => service.Unban("NotBanned"));
        }
    }
}
