using System;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Noobietoria.Platform;

/// <summary>
/// Offline-verifiable join ticket issued by the Noobietoria platform
/// backend (closed source): base64url(payload) + "." + base64url(HMAC-SHA256).
///
/// The payload carries the account, the target world and an expiry; game
/// servers verify the signature with the shared fleet secret, so a place
/// accepts players on the hot path without calling the backend. The real
/// issuing service runs on the platform (Cloudflare Workers); the dev
/// ticket secret below only exists so the open client/server can be tested
/// without the backend.
/// </summary>
public static class JoinTicket
{
    private const string FormatVersion = "nj1";

    /// <summary>Payload carried inside a ticket.</summary>
    public sealed record Payload(string Username, long UserId, string WorldId, long ExpiresAtUnixSeconds);

    private sealed record TicketDocument(string V, string U, long Uid, string W, long Exp);

    /// <summary>Signs a payload into a ticket.</summary>
    public static string Issue(Payload payload, ReadOnlySpan<byte> secret)
    {
        var document = new TicketDocument(FormatVersion, payload.Username, payload.UserId,
            payload.WorldId, payload.ExpiresAtUnixSeconds);
        byte[] payloadBytes = JsonSerializer.SerializeToUtf8Bytes(document);
        byte[] signature = Sign(payloadBytes, secret);

        return Base64UrlEncode(payloadBytes) + "." + Base64UrlEncode(signature);
    }

    /// <summary>
    /// Validates a ticket: signature, format version and expiry. Returns the
    /// payload, or null when the ticket is invalid/expired.
    /// </summary>
    public static Payload? Validate(string? ticket, ReadOnlySpan<byte> secret, DateTime? utcNow = null)
    {
        if (string.IsNullOrWhiteSpace(ticket))
            return null;

        int separator = ticket.IndexOf('.', StringComparison.Ordinal);
        if (separator <= 0 || separator == ticket.Length - 1)
            return null;

        byte[] payloadBytes;
        byte[] signature;
        try
        {
            payloadBytes = Base64UrlDecode(ticket[..separator]);
            signature = Base64UrlDecode(ticket[(separator + 1)..]);
        }
        catch (FormatException)
        {
            return null;
        }

        byte[] expected = Sign(payloadBytes, secret);
        if (!CryptographicOperations.FixedTimeEquals(signature, expected))
            return null;

        TicketDocument? document;
        try
        {
            document = JsonSerializer.Deserialize<TicketDocument>(payloadBytes);
        }
        catch (JsonException)
        {
            return null;
        }
        if (document == null || document.V != FormatVersion)
            return null;

        long now = new DateTimeOffset(utcNow ?? DateTime.UtcNow).ToUnixTimeSeconds();
        if (document.Exp <= now)
            return null;

        return new Payload(document.U, document.Uid, document.W, document.Exp);
    }

    /// <summary>Dev ticket secret — lets the open client/server run without the closed backend.</summary>
    public static byte[] DevSecret() => Encoding.UTF8.GetBytes("dev-only-noobietoria-ticket-secret");

    private static byte[] Sign(byte[] payload, ReadOnlySpan<byte> secret)
    {
        using var hmac = new HMACSHA256(secret.ToArray());
        return hmac.ComputeHash(payload);
    }

    internal static string Base64UrlEncode(byte[] data) =>
        Convert.ToBase64String(data).TrimEnd('=').Replace('+', '-').Replace('/', '_');

    internal static byte[] Base64UrlDecode(string value)
    {
        string padded = value.Replace('-', '+').Replace('_', '/');
        return Convert.FromBase64String(padded + new string('=', (4 - padded.Length % 4) % 4));
    }
}
