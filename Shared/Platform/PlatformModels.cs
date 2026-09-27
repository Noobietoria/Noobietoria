using System.Collections.Generic;

namespace Noobietoria.Platform;

/// <summary>A platform account after login/registration.</summary>
public sealed record PlatformAccount(long UserId, string Username, string Token);

/// <summary>A game entry in the platform catalog (portal listing).</summary>
public sealed record GameListing(string GameId, string Title, string Description, string WorldId, int PlayersOnline);

/// <summary>Where to connect after pressing Play (platform-assigned place).</summary>
public sealed record JoinResponse(string Host, int Port, string Ticket, string WorldId);

/// <summary>A world definition the fleet can host (server side).</summary>
public sealed record WorldDefinition(string WorldId, string Name, string Description,
    double SpawnX, double SpawnY, double SpawnZ, double SpawnRadius);
