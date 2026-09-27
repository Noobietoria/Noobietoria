using System;
using System.Collections.Generic;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
using Noobietoria.Studio.Core;
using Xunit;

namespace Noobietoria.Studio.Tests
{
    public class HttpServiceTests
    {
        /// <summary>
        /// Deterministic in-memory handler: records every request and replies
        /// from a configurable responder, so no test touches the network.
        /// </summary>
        private sealed class FakeHandler : HttpMessageHandler
        {
            public List<HttpRequestMessage> Requests { get; } = new();
            public List<string?> Bodies { get; } = new();

            public Func<HttpRequestMessage, HttpResponseMessage> Responder { get; set; } =
                _ => new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent("ok") };

            protected override Task<HttpResponseMessage> SendAsync(
                HttpRequestMessage request, CancellationToken cancellationToken)
            {
                Requests.Add(request);
                Bodies.Add(request.Content == null
                    ? null
                    : request.Content.ReadAsStringAsync(cancellationToken).GetAwaiter().GetResult());
                return Task.FromResult(Responder(request));
            }
        }

        [Fact]
        public async Task Prepare_Request_Release_Lifecycle()
        {
            var handler = new FakeHandler();
            var service = new HttpService(handler);

            service.Prepare(1, "GET", "https://example.test/ping");
            var response = await service.Request(1);

            Assert.True(response.IsSuccess);
            Assert.Equal(200, response.StatusCode);
            Assert.Equal("ok", response.Body);

            service.Release(1);

            await Assert.ThrowsAsync<InvalidOperationException>(async () => await service.Request(1));
        }

        [Fact]
        public async Task PreparedRequest_CanBeRequestedRepeatedly()
        {
            var handler = new FakeHandler();
            var service = new HttpService(handler);
            service.Prepare(1, "GET", "https://example.test/ping");

            await service.Request(1);
            await service.Request(1);

            Assert.Equal(2, handler.Requests.Count);
        }

        [Theory]
        [InlineData("get", "GET")]
        [InlineData("post", "POST")]
        [InlineData("put", "PUT")]
        [InlineData("delete", "DELETE")]
        [InlineData("head", "HEAD")]
        [InlineData("options", "OPTIONS")]
        public async Task Prepare_SupportsDocumentedMethods(string given, string expected)
        {
            var handler = new FakeHandler();
            var service = new HttpService(handler);

            service.Prepare(1, given, "https://example.test/");
            await service.Request(1);

            Assert.Equal(expected, handler.Requests.Single().Method.Method);
        }

        [Theory]
        [InlineData("TRACE")]
        [InlineData("PATCH")]
        [InlineData("")]
        public void Prepare_RejectsUnsupportedMethods(string method)
        {
            var service = new HttpService(new FakeHandler());

            Assert.Throws<ArgumentException>(() => service.Prepare(1, method, "https://example.test/"));
        }

        [Theory]
        [InlineData("example.test/no-scheme")]
        [InlineData("ftp://example.test/")]
        [InlineData("")]
        public void Prepare_RejectsNonHttpUrls(string url)
        {
            var service = new HttpService(new FakeHandler());

            Assert.Throws<ArgumentException>(() => service.Prepare(1, "GET", url));
        }

        [Fact]
        public void Prepare_SameIdTwice_Throws()
        {
            var service = new HttpService(new FakeHandler());

            service.Prepare(7, "GET", "https://example.test/");

            Assert.Throws<InvalidOperationException>(() =>
                service.Prepare(7, "GET", "https://example.test/other"));
        }

        [Fact]
        public void Prepare_AllowsDocumentedContentTypes_AndRejectsOthers()
        {
            var service = new HttpService(new FakeHandler());

            service.Prepare(1, "POST", "https://example.test/json", null, "{\"a\":1}", "application/json");
            service.Prepare(2, "POST", "https://example.test/form", null, "a=1&b=2",
                "application/x-www-form-urlencoded");
            service.Prepare(3, "POST", "https://example.test/upload", null, "--x--",
                "multipart/form-data; boundary=x");
            service.Prepare(4, "POST", "https://example.test/params", null, "{\"a\":1}",
                "application/json; charset=utf-8");

            Assert.Throws<ArgumentException>(() =>
                service.Prepare(5, "POST", "https://example.test/", null, "hello", "text/html"));
        }

        [Fact]
        public void Prepare_BodyWithoutContentType_Throws()
        {
            var service = new HttpService(new FakeHandler());

            Assert.Throws<ArgumentException>(() =>
                service.Prepare(1, "POST", "https://example.test/", null, "raw body"));
        }

        [Fact]
        public async Task Request_PassesThroughHeadersBodyAndContentType()
        {
            var handler = new FakeHandler();
            var service = new HttpService(handler);
            var headers = new Dictionary<string, string> { ["X-Api-Key"] = "secret", ["Accept"] = "application/json" };

            service.Prepare(1, "POST", "https://example.test/api",
                headers, "{\"a\":1}", "application/json");
            await service.Request(1);

            var sent = handler.Requests.Single();
            Assert.Equal("secret", Assert.Single(sent.Headers.GetValues("X-Api-Key")));
            Assert.Equal("{\"a\":1}", handler.Bodies.Single());
            Assert.NotNull(sent.Content?.Headers.ContentType);
            Assert.Equal("application/json", sent.Content!.Headers.ContentType!.MediaType);
        }

        [Fact]
        public async Task Request_ReportsNonSuccessStatuses()
        {
            var handler = new FakeHandler
            {
                Responder = _ => new HttpResponseMessage(HttpStatusCode.InternalServerError)
                {
                    Content = new StringContent("boom"),
                },
            };
            var service = new HttpService(handler);
            service.Prepare(1, "GET", "https://example.test/fail");

            var response = await service.Request(1);

            Assert.False(response.IsSuccess);
            Assert.Equal(500, response.StatusCode);
            Assert.Equal("boom", response.Body);
        }

        [Fact]
        public async Task Response_Headers_AreExposed()
        {
            var handler = new FakeHandler
            {
                Responder = _ =>
                {
                    var response = new HttpResponseMessage(HttpStatusCode.OK)
                    {
                        Content = new StringContent("ok"),
                    };
                    response.Headers.Add("X-RateLimit-Remaining", "99");
                    return response;
                },
            };
            var service = new HttpService(handler);
            service.Prepare(1, "GET", "https://example.test/");

            var response = await service.Request(1);

            Assert.Equal("99", response.Headers["X-RateLimit-Remaining"]);
        }

        [Fact]
        public async Task Request_UnknownId_Throws()
        {
            var service = new HttpService(new FakeHandler());

            await Assert.ThrowsAsync<InvalidOperationException>(async () => await service.Request(99));
        }

        [Fact]
        public void Release_UnknownId_Throws()
        {
            var service = new HttpService(new FakeHandler());

            Assert.Throws<InvalidOperationException>(() => service.Release(42));
        }

        [Fact]
        public async Task Head_Request_HasEmptyBody()
        {
            var handler = new FakeHandler
            {
                Responder = _ => new HttpResponseMessage(HttpStatusCode.OK), // no content
            };
            var service = new HttpService(handler);
            service.Prepare(1, "HEAD", "https://example.test/");

            var response = await service.Request(1);

            Assert.True(response.IsSuccess);
            Assert.Equal(string.Empty, response.Body);
        }
    }
}
