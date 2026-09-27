using System;
using System.Collections.Generic;
using System.Net.Http;
using System.Text;
using System.Threading.Tasks;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// HttpService sends HTTP requests to external web services from server
    /// scripts. Requests are prepared up-front and executed explicitly:
    ///
    ///   HttpService.Prepare(Id, Method, Url, Headers, Body, ContentType)
    ///   HttpService.Request(Id)   // returns an HttpResponse
    ///   HttpService.Release(Id)   // frees the prepared request
    ///
    /// Methods: GET, POST, PUT, DELETE, HEAD, OPTIONS.
    /// Content-Types: URL-encoded params (application/x-www-form-urlencoded),
    /// JSON (application/json), file upload (multipart/form-data).
    /// A prepared Id can be requested repeatedly; Release frees it. All
    /// contract violations (unknown Id, bad method/URL/content-type) throw.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#httpservice
    /// </summary>
    public class HttpService
    {
        private static readonly HashSet<string> AllowedMethods = new(StringComparer.OrdinalIgnoreCase)
        {
            "GET",
            "POST",
            "PUT",
            "DELETE",
            "HEAD",
            "OPTIONS",
        };

        private static readonly HashSet<string> AllowedContentTypes = new(StringComparer.OrdinalIgnoreCase)
        {
            "application/x-www-form-urlencoded",
            "application/json",
            "multipart/form-data",
        };

        /// <summary>A prepared request, identified by its Id.</summary>
        public sealed class PreparedRequest
        {
            public int Id { get; }
            public string Method { get; }
            public string Url { get; }
            public IReadOnlyDictionary<string, string> Headers { get; }
            public string Body { get; }
            public string ContentType { get; }

            internal PreparedRequest(
                int id, string method, string url,
                IReadOnlyDictionary<string, string> headers, string body, string contentType)
            {
                Id = id;
                Method = method;
                Url = url;
                Headers = headers;
                Body = body;
                ContentType = contentType;
            }
        }

        /// <summary>The outcome of HttpService.Request(Id).</summary>
        public sealed record HttpResponse(
            int StatusCode,
            bool IsSuccess,
            string Body,
            IReadOnlyDictionary<string, string> Headers);

        private readonly object _lock = new();
        private readonly Dictionary<int, PreparedRequest> _prepared = new();
        private readonly HttpClient _client;

        /// <summary>
        /// Creates the service. A custom handler can be injected for tests;
        /// the default timeout is 30 seconds.
        /// </summary>
        public HttpService(HttpMessageHandler? handler = null, TimeSpan? timeout = null)
        {
            _client = handler != null
                ? new HttpClient(handler) { Timeout = timeout ?? DefaultTimeout }
                : new HttpClient { Timeout = timeout ?? DefaultTimeout };
        }

        private static readonly TimeSpan DefaultTimeout = TimeSpan.FromSeconds(30);

        /// <summary>Disposes the underlying HttpClient.</summary>
        public void Dispose() => _client.Dispose();

        /// <summary>
        /// HttpService.Prepare(Id, Method, Url, Headers, Body, ContentType) —
        /// registers a request under Id. Throws when the method, URL or
        /// content type is not supported, or when Id is already in use.
        /// </summary>
        public void Prepare(
            int id,
            string method,
            string url,
            IReadOnlyDictionary<string, string>? headers = null,
            string? body = null,
            string? contentType = null)
        {
            if (string.IsNullOrWhiteSpace(method))
                throw new ArgumentException("Method must not be empty.", nameof(method));

            string normalized = method.Trim().ToUpperInvariant();
            if (!AllowedMethods.Contains(normalized))
                throw new ArgumentException(
                    $"Method '{method}' is not supported. Allowed: {string.Join(", ", AllowedMethods)}.", nameof(method));

            if (string.IsNullOrWhiteSpace(url)
                || !Uri.TryCreate(url.Trim(), UriKind.Absolute, out var uri)
                || (uri.Scheme != Uri.UriSchemeHttp && uri.Scheme != Uri.UriSchemeHttps))
                throw new ArgumentException(
                    "Url must be an absolute http(s) URL.", nameof(url));

            string? baseContentType = null;
            if (!string.IsNullOrWhiteSpace(contentType))
            {
                baseContentType = contentType.Split(';')[0].Trim();
                if (!AllowedContentTypes.Contains(baseContentType))
                    throw new ArgumentException(
                        $"ContentType '{contentType}' is not supported. Allowed: URL-encoded params, JSON, file upload.",
                        nameof(contentType));
            }

            if (!string.IsNullOrEmpty(body) && baseContentType == null)
                throw new ArgumentException(
                    "ContentType is required when a Body is provided (URL-encoded params, JSON or file upload).",
                    nameof(contentType));

            var snapshot = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            if (headers != null)
            {
                foreach (var kv in headers)
                    snapshot[kv.Key] = kv.Value;
            }

            lock (_lock)
            {
                if (_prepared.ContainsKey(id))
                    throw new InvalidOperationException(
                        $"Request Id {id} is already prepared — Release it before preparing it again.");

                _prepared[id] = new PreparedRequest(
                    id, normalized, url.Trim(), snapshot, body ?? string.Empty, contentType ?? string.Empty);
            }
        }

        /// <summary>
        /// HttpService.Request(Id) — executes a prepared request. Can be
        /// called repeatedly until the request is released.
        /// </summary>
        public async Task<HttpResponse> Request(int id)
        {
            PreparedRequest prepared;
            lock (_lock)
            {
                if (!_prepared.TryGetValue(id, out var slot))
                    throw new InvalidOperationException(
                        $"No prepared request with Id {id}. Call Prepare first.");
                prepared = slot;
            }

            using var message = new HttpRequestMessage(new HttpMethod(prepared.Method), prepared.Url);
            foreach (var kv in prepared.Headers)
                message.Headers.TryAddWithoutValidation(kv.Key, kv.Value);

            if (prepared.Body.Length > 0)
            {
                var content = new StringContent(prepared.Body, Encoding.UTF8);
                content.Headers.ContentType = null;
                if (prepared.ContentType.Length > 0)
                    content.Headers.TryAddWithoutValidation("Content-Type", prepared.ContentType);
                message.Content = content;
            }

            using var response = await _client.SendAsync(message).ConfigureAwait(false);

            var headers = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            foreach (var header in response.Headers)
                headers[header.Key] = string.Join(", ", header.Value);
            if (response.Content is not null)
            {
                foreach (var header in response.Content.Headers)
                    headers[header.Key] = string.Join(", ", header.Value);
            }

            string body = response.Content is null
                ? string.Empty
                : await response.Content.ReadAsStringAsync().ConfigureAwait(false);
            int status = (int)response.StatusCode;

            return new HttpResponse(status, status is >= 200 and <= 299, body, headers);
        }

        /// <summary>
        /// HttpService.Release(Id) — frees the prepared request. Requesting
        /// it afterwards throws.
        /// </summary>
        public void Release(int id)
        {
            lock (_lock)
            {
                if (!_prepared.Remove(id))
                    throw new InvalidOperationException($"No prepared request with Id {id}.");
            }
        }
    }
}
