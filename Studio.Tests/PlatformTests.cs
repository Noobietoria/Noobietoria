using System;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using Noobietoria.Platform;
using Xunit;

namespace Noobietoria.Studio.Tests
{
    public class JoinTicketTests
    {
        private static readonly byte[] Secret = Encoding.UTF8.GetBytes("fleet-secret");
        private static readonly DateTime Now = new(2026, 1, 1, 12, 0, 0, DateTimeKind.Utc);

        private static JoinTicket.Payload SamplePayload(
            string username = "Alice", long userId = 7, string worldId = "grasslands") =>
            new(username, userId, worldId, new DateTimeOffset(Now.AddHours(1)).ToUnixTimeSeconds());

        [Fact]
        public void Issue_Validate_RoundTrips()
        {
            string ticket = JoinTicket.Issue(SamplePayload(), Secret);

            var payload = JoinTicket.Validate(ticket, Secret, Now);
            Assert.NotNull(payload);
            Assert.Equal("Alice", payload!.Username);
            Assert.Equal(7, payload.UserId);
            Assert.Equal("grasslands", payload.WorldId);
        }

        [Fact]
        public void Validate_RejectsTamperedTickets()
        {
            string ticket = JoinTicket.Issue(SamplePayload(), Secret);

            Assert.Null(JoinTicket.Validate(ticket, Encoding.UTF8.GetBytes("other-secret"), Now));

            string[] parts = ticket.Split('.');
            string flipped = parts[0][..^2] + "xx." + parts[1];
            Assert.Null(JoinTicket.Validate(flipped, Secret, Now));
        }

        [Fact]
        public void Validate_RejectsExpiredTickets()
        {
            string ticket = JoinTicket.Issue(SamplePayload(), Secret);

            Assert.Null(JoinTicket.Validate(ticket, Secret, Now.AddHours(2)));
        }

        [Fact]
        public void Validate_RejectsGarbage()
        {
            Assert.Null(JoinTicket.Validate("", Secret, Now));
            Assert.Null(JoinTicket.Validate("not-a-ticket", Secret, Now));
            Assert.Null(JoinTicket.Validate("###.###", Secret, Now));
        }
    }

    public class PlatformClientTests
    {
        private sealed class FakeHandler : HttpMessageHandler
        {
            public string? LastUrl;
            public string? LastBody;
            public System.Net.Http.Headers.AuthenticationHeaderValue? Auth;

            public Func<HttpRequestMessage, HttpResponseMessage> Responder { get; set; } =
                _ => new HttpResponseMessage(HttpStatusCode.OK)
                {
                    Content = new StringContent("{\"userId\":1,\"username\":\"u\",\"token\":\"t\"}"),
                };

            protected override Task<HttpResponseMessage> SendAsync(
                HttpRequestMessage request, CancellationToken cancellationToken)
            {
                LastUrl = request.RequestUri!.ToString();
                Auth = request.Headers.Authorization;
                LastBody = request.Content == null
                    ? null
                    : request.Content.ReadAsStringAsync(cancellationToken).GetAwaiter().GetResult();
                return Task.FromResult(Responder(request));
            }
        }

        [Fact]
        public async Task Online_Login_Games_Join()
        {
            var handler = new FakeHandler();
            handler.Responder = _ => new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("{\"userId\":1,\"username\":\"alice\",\"token\":\"t\"}"),
            };
            var service = new PlatformClient("https://api.test", handler);

            var account = await service.LoginAsync("alice", "pw");
            Assert.Equal("https://api.test/v1/accounts/login", handler.LastUrl);
            Assert.Equal("alice", account.Username);

            handler.Responder = _ => new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent(
                    "[{\"gameId\":\"g1\",\"title\":\"Game One\",\"description\":\"d\",\"worldId\":\"grasslands\",\"playersOnline\":3}]"),
            };
            var games = await service.GetGamesAsync(account.Token);
            Assert.Equal("Game One", games.Single().Title);
            Assert.Equal("Bearer", handler.Auth!.Scheme);

            handler.Responder = _ => new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent(
                    "{\"host\":\"10.0.0.1\",\"port\":24565,\"ticket\":\"tk\",\"worldId\":\"grasslands\"}"),
            };
            var join = await service.RequestJoinAsync("g1", account.Token);
            Assert.Equal(24565, join.Port);
            Assert.Equal("tk", join.Ticket);
        }

        [Fact]
        public async Task Online_Error_Throws()
        {
            var handler = new FakeHandler
            {
                Responder = _ => new HttpResponseMessage(HttpStatusCode.Unauthorized),
            };
            var service = new PlatformClient("https://api.test", handler);

            await Assert.ThrowsAsync<InvalidOperationException>(
                async () => await service.LoginAsync("alice", "bad"));
        }

        [Fact]
        public async Task Offline_Login_Catalog_AndJoin_WorkWithoutBackend()
        {
            var service = new PlatformClient(); // offline dev mode
            Assert.True(service.OfflineMode);

            var account = await service.LoginAsync("Alice", "whatever");
            Assert.Equal("Alice", account.Username);
            Assert.NotEmpty(account.Token);

            var games = await service.GetGamesAsync(account.Token);
            Assert.True(games.Count >= 2);
            Assert.Contains(games, g => g.WorldId == "grasslands");

            var join = await service.RequestJoinAsync(games.First().GameId, account.Token);
            Assert.Equal("127.0.0.1", join.Host);
            Assert.Equal(24565, join.Port);

            // The offline join ticket must verify against the dev secret.
            var payload = JoinTicket.Validate(join.Ticket, JoinTicket.DevSecret(), DateTime.UtcNow);
            Assert.NotNull(payload);
            Assert.Equal(games.First().WorldId, payload!.WorldId);
        }

        [Fact]
        public async Task Offline_UnknownGame_Throws()
        {
            var service = new PlatformClient(null, new FakeHandler());
            await Assert.ThrowsAsync<InvalidOperationException>(
                async () => await service.RequestJoinAsync("missing", "t"));
        }
    }
}
