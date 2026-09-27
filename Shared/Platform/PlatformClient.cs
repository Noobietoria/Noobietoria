using System;
using System.Collections.Generic;
using System.Linq;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Threading.Tasks;

namespace Noobietoria.Platform;

/// <summary>
/// HTTP client for the Noobietoria platform backend (closed source,
/// Cloudflare Workers + D1/KV on the platform side). The wire contract is
/// documented in platform/openapi.yaml.
///
/// With no base URL the client runs in offline dev mode: any credentials
/// log in, a built-in sample catalog is returned, and join tickets are
/// signed with the dev secret — so the open client/server can be played
/// without the backend. The closed backend implements the exact same
/// contract for production.
/// </summary>
public class PlatformClient
{
    /// <summary>Game catalog served in offline dev mode (world ids match DedicatedServer/worlds).</summary>
    public static readonly IReadOnlyList<GameListing> DevCatalog = new[]
    {
        new GameListing("grasslands-social", "Grasslands Social",
            "Hang out, chat and explore the rolling hills.", "grasslands", 0),
        new GameListing("moonbase-social", "Moonbase Social",
            "Low-gravity hangout inside the lunar dome.", "moonbase", 0),
    };

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        PropertyNameCaseInsensitive = true,
    };

    private readonly HttpClient _http;
    private readonly string? _baseUrl;

    /// <param name="baseUrl">
    /// Platform API base URL (e.g. "https://api.noobietoria.dev"). Pass null
    /// or an empty string to run in offline dev mode.
    /// </param>
    public PlatformClient(string? baseUrl = null, HttpMessageHandler? handler = null)
    {
        _baseUrl = string.IsNullOrWhiteSpace(baseUrl) ? null : baseUrl.Trim().TrimEnd('/');
        _http = new HttpClient(handler ?? new HttpClientHandler()) { Timeout = TimeSpan.FromSeconds(15) };
    }

    /// <summary>True when running without the real backend.</summary>
    public bool OfflineMode => _baseUrl == null;

    /// <summary>Accounts:Login(Username, Password) — register a new platform account.</summary>
    public async Task<PlatformAccount> RegisterAsync(string username, string password)
    {
        if (OfflineMode)
            return OfflineAccount(username);

        using var content = JsonContent("{\"username\":\"" + Escape(username) + "\",\"password\":\"" + Escape(password) + "\"}");
        using var response = await _http.PostAsync(_baseUrl + "/v1/accounts/register", content).ConfigureAwait(false);
        return await ReadAccountAsync(response).ConfigureAwait(false);
    }

    /// <summary>Accounts:Login(Username, Password)</summary>
    public async Task<PlatformAccount> LoginAsync(string username, string password)
    {
        if (OfflineMode)
            return OfflineAccount(username);

        using var content = JsonContent("{\"username\":\"" + Escape(username) + "\",\"password\":\"" + Escape(password) + "\"}");
        using var response = await _http.PostAsync(_baseUrl + "/v1/accounts/login", content).ConfigureAwait(false);
        return await ReadAccountAsync(response).ConfigureAwait(false);
    }

    /// <summary>Catalog: the list of playable games.</summary>
    public async Task<IReadOnlyList<GameListing>> GetGamesAsync(string? token)
    {
        if (OfflineMode)
            return DevCatalog;

        using var request = new HttpRequestMessage(HttpMethod.Get, _baseUrl + "/v1/games");
        request.Headers.Authorization = new System.Net.Http.Headers.AuthenticationHeaderValue("Bearer", token ?? "");
        using var response = await _http.SendAsync(request).ConfigureAwait(false);
        EnsureSuccess(response);

        var body = await response.Content.ReadAsStringAsync().ConfigureAwait(false);
        return JsonSerializer.Deserialize<List<GameListing>>(body, JsonOptions) ?? new List<GameListing>();
    }

    /// <summary>
    /// Session: asks the platform for a place assignment and a signed join
    /// ticket for the chosen game.
    /// </summary>
    public async Task<JoinResponse> RequestJoinAsync(string gameId, string? token)
    {
        if (OfflineMode)
        {
            var listing = DevCatalog.FirstOrDefault(g => g.GameId == gameId)
                ?? throw new InvalidOperationException($"Unknown game '{gameId}'.");
            long expires = DateTimeOffset.UtcNow.AddHours(12).ToUnixTimeSeconds();
            string ticket = JoinTicket.Issue(
                new JoinTicket.Payload(listing.Title, 0, listing.WorldId, expires),
                JoinTicket.DevSecret());
            return new JoinResponse("127.0.0.1", Noobietoria.Shared.Net.DefaultPort, ticket, listing.WorldId);
        }

        using var request = new HttpRequestMessage(HttpMethod.Post, _baseUrl + "/v1/games/" + Uri.EscapeDataString(gameId) + "/join");
        request.Headers.Authorization = new System.Net.Http.Headers.AuthenticationHeaderValue("Bearer", token ?? "");
        using var response = await _http.SendAsync(request).ConfigureAwait(false);
        EnsureSuccess(response);

        var body = await response.Content.ReadAsStringAsync().ConfigureAwait(false);
        return JsonSerializer.Deserialize<JoinResponse>(body, JsonOptions)
            ?? throw new InvalidOperationException("Platform returned an empty join response.");
    }

    // ---- offline dev mode ----

    private static PlatformAccount OfflineAccount(string username)
    {
        if (string.IsNullOrWhiteSpace(username))
            throw new ArgumentException("Username must not be empty.", nameof(username));

        long userId = Fnv1a(username.Trim());
        long expires = DateTimeOffset.UtcNow.AddHours(12).ToUnixTimeSeconds();
        string token = JoinTicket.Issue(
            new JoinTicket.Payload(username.Trim(), userId, "", expires),
            JoinTicket.DevSecret());
        return new PlatformAccount(userId, username.Trim(), token);
    }

    private static long Fnv1a(string value)
    {
        unchecked
        {
            ulong hash = 14695981039346656037;
            foreach (char c in value)
            {
                hash ^= c;
                hash *= 1099511628211;
            }
            return (long)(hash & 0x7FFFFFFFFFFFFFFF);
        }
    }

    // ---- HTTP helpers ----

    private static StringContent JsonContent(string json) =>
        new(json, Encoding.UTF8, "application/json");

    private static string Escape(string value) =>
        JsonSerializer.Serialize(value);

    private static async Task<PlatformAccount> ReadAccountAsync(HttpResponseMessage response)
    {
        EnsureSuccess(response);
        var body = await response.Content.ReadAsStringAsync().ConfigureAwait(false);
        return JsonSerializer.Deserialize<PlatformAccount>(body, JsonOptions)
            ?? throw new InvalidOperationException("Platform returned an empty account response.");
    }

    private static void EnsureSuccess(HttpResponseMessage response)
    {
        if (!response.IsSuccessStatusCode)
            throw new InvalidOperationException($"Platform request failed: {(int)response.StatusCode} {response.ReasonPhrase}.");
    }
}
