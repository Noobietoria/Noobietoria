using System;
using System.Collections.Generic;
using System.Linq;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// AchievementService registers achievements and tracks per-player
    /// progress; reaching MaxProgress unlocks the achievement (idempotent).
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#achievementservice
    /// </summary>
    public class AchievementService
    {
        /// <summary>An achievement definition (GetAll payload).</summary>
        public sealed record AchievementSpec(string AchievementId, string Name, string Description, double MaxProgress);

        private sealed record Progress(double Value, bool Unlocked);

        private readonly object _lock = new();
        private readonly Dictionary<string, AchievementSpec> _definitions = new(StringComparer.Ordinal);
        private readonly Dictionary<string, Progress> _progress = new(StringComparer.Ordinal); // "user\u0001id"
        private readonly Dictionary<string, List<Action<string>>> _unlockCallbacks = new(StringComparer.Ordinal);

        /// <summary>AchievementService.Register(AchievementId, Name, Description, MaxProgress)</summary>
        public void Register(string achievementId, string name, string description, double maxProgress)
        {
            RequireId(achievementId);
            if (string.IsNullOrWhiteSpace(name))
                throw new ArgumentException("Name must not be empty.", nameof(name));
            if (maxProgress <= 0)
                throw new ArgumentException("MaxProgress must be greater than 0.", nameof(maxProgress));

            lock (_lock)
            {
                if (!_definitions.TryAdd(achievementId, new AchievementSpec(achievementId, name, description, maxProgress)))
                    throw new InvalidOperationException($"Achievement '{achievementId}' is already registered.");
            }
        }

        /// <summary>AchievementService.Unregister(AchievementId)</summary>
        public void Unregister(string achievementId)
        {
            RequireId(achievementId);
            lock (_lock)
            {
                if (!_definitions.Remove(achievementId))
                    throw new InvalidOperationException($"No achievement '{achievementId}'.");
            }
        }

        /// <summary>AchievementService.GetAll()</summary>
        public IReadOnlyList<AchievementSpec> GetAll()
        {
            lock (_lock)
            {
                return _definitions.Values.ToArray();
            }
        }

        /// <summary>AchievementService.GetProgress(Username, AchievementId)</summary>
        public double GetProgress(string username, string achievementId)
        {
            RequireDefinition(achievementId);
            lock (_lock)
            {
                return ProgressKey(username, achievementId) is { } key && _progress.TryGetValue(key, out var p)
                    ? p.Value
                    : 0;
            }
        }

        /// <summary>AchievementService.SetProgress(Username, AchievementId, Progress)</summary>
        public void SetProgress(string username, string achievementId, double progress)
        {
            RequireDefinition(achievementId);
            var spec = _definitions.GetValueOrDefault(achievementId)!;
            if (progress < 0 || progress > spec.MaxProgress)
                throw new ArgumentException(
                    $"Progress must be between 0 and MaxProgress ({spec.MaxProgress}).", nameof(progress));
            Store(username, spec, progress);
        }

        /// <summary>AchievementService.AddProgress(Username, AchievementId, Delta)</summary>
        public void AddProgress(string username, string achievementId, double delta)
        {
            RequireDefinition(achievementId);
            var spec = _definitions.GetValueOrDefault(achievementId)!;
            double current = GetProgress(username, achievementId);
            double target = Math.Clamp(current + delta, 0, spec.MaxProgress);
            Store(username, spec, target);
        }

        /// <summary>AchievementService.Unlock(Username, AchievementId) — idempotent.</summary>
        public void Unlock(string username, string achievementId)
        {
            RequireDefinition(achievementId);
            var spec = _definitions.GetValueOrDefault(achievementId)!;
            List<Action<string>>? callbacks;
            lock (_lock)
            {
                string key = ProgressKey(username, achievementId);
                var progress = _progress.TryGetValue(key, out var existing) ? existing : new Progress(0, false);
                if (progress.Unlocked)
                    return;
                _progress[key] = progress with { Unlocked = true, Value = spec.MaxProgress };
                _unlockCallbacks.TryGetValue(username, out callbacks);
            }
            callbacks?.ForEach(cb => cb(achievementId));
        }

        /// <summary>AchievementService.IsUnlocked(Username, AchievementId)</summary>
        public bool IsUnlocked(string username, string achievementId)
        {
            RequireDefinition(achievementId);
            lock (_lock)
            {
                return ProgressKey(username, achievementId) is { } key && _progress.TryGetValue(key, out var p)
                    && p.Unlocked;
            }
        }

        /// <summary>AchievementService.GetUnlocked(Username)</summary>
        public IReadOnlyList<string> GetUnlocked(string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            lock (_lock)
            {
                return _definitions.Keys
                    .Where(id => ProgressKey(username, id) is { } key && _progress.TryGetValue(key, out var p) && p.Unlocked)
                    .ToArray();
            }
        }

        /// <summary>AchievementService.OnUnlock(Username, Callback) — receives the AchievementId.</summary>
        public void OnUnlock(string username, Action<string> callback)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                if (!_unlockCallbacks.TryGetValue(username, out var list))
                    _unlockCallbacks[username] = list = new List<Action<string>>();
                list.Add(callback);
            }
        }

        private void Store(string username, AchievementSpec spec, double progress)
        {
            List<Action<string>>? callbacks;
            bool unlocked;
            lock (_lock)
            {
                string key = ProgressKey(username, spec.AchievementId);
                var current = _progress.TryGetValue(key, out var existing) ? existing : new Progress(0, false);
                unlocked = current.Unlocked || progress >= spec.MaxProgress;
                _progress[key] = new Progress(progress, unlocked);
                _unlockCallbacks.TryGetValue(username, out callbacks);
            }
            if (unlocked)
                callbacks?.ForEach(cb => cb(spec.AchievementId));
        }

        private AchievementSpec RequireDefinition(string achievementId)
        {
            RequireId(achievementId);
            return _definitions.GetValueOrDefault(achievementId)
                ?? throw new InvalidOperationException($"No achievement '{achievementId}'.");
        }

        private static string ProgressKey(string username, string achievementId) =>
            username + "\u0001" + achievementId;

        private static void RequireId(string achievementId)
        {
            if (string.IsNullOrWhiteSpace(achievementId))
                throw new ArgumentException("AchievementId must not be empty.", nameof(achievementId));
        }
    }

    /// <summary>
    /// BadgeService registers badges and awards/revokes them per player.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#badgeservice
    /// </summary>
    public class BadgeService
    {
        /// <summary>A badge definition (GetAll payload).</summary>
        public sealed record BadgeSpec(string BadgeId, string Name, string Description);

        private readonly object _lock = new();
        private readonly Dictionary<string, BadgeSpec> _definitions = new(StringComparer.Ordinal);
        private readonly Dictionary<string, HashSet<string>> _awarded = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action<string>>> _awardCallbacks = new(StringComparer.Ordinal);

        /// <summary>BadgeService.Register(BadgeId, Name, Description)</summary>
        public void Register(string badgeId, string name, string description)
        {
            RequireId(badgeId);
            if (string.IsNullOrWhiteSpace(name))
                throw new ArgumentException("Name must not be empty.", nameof(name));
            lock (_lock)
            {
                if (!_definitions.TryAdd(badgeId, new BadgeSpec(badgeId, name, description)))
                    throw new InvalidOperationException($"Badge '{badgeId}' is already registered.");
            }
        }

        /// <summary>BadgeService.Unregister(BadgeId)</summary>
        public void Unregister(string badgeId)
        {
            RequireId(badgeId);
            lock (_lock)
            {
                if (!_definitions.Remove(badgeId))
                    throw new InvalidOperationException($"No badge '{badgeId}'.");
            }
        }

        /// <summary>BadgeService.Award(Username, BadgeId) — idempotent; OnAward fires on first award.</summary>
        public void Award(string username, string badgeId)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            RequireDefinition(badgeId);

            List<Action<string>>? callbacks = null;
            lock (_lock)
            {
                if (!_awarded.TryGetValue(username, out var set))
                    _awarded[username] = set = new HashSet<string>(StringComparer.Ordinal);
                if (set.Add(badgeId))
                    _awardCallbacks.TryGetValue(username, out callbacks);
            }
            callbacks?.ForEach(cb => cb(badgeId));
        }

        /// <summary>BadgeService.Revoke(Username, BadgeId)</summary>
        public void Revoke(string username, string badgeId)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            RequireDefinition(badgeId);
            lock (_lock)
            {
                _awarded.GetValueOrDefault(username)?.Remove(badgeId);
            }
        }

        /// <summary>BadgeService.HasBadge(Username, BadgeId)</summary>
        public bool HasBadge(string username, string badgeId)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            RequireDefinition(badgeId);
            lock (_lock)
            {
                return _awarded.GetValueOrDefault(username)?.Contains(badgeId) == true;
            }
        }

        /// <summary>BadgeService.GetBadges(Username)</summary>
        public IReadOnlyList<string> GetBadges(string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            lock (_lock)
            {
                return _awarded.GetValueOrDefault(username)?.OrderBy(b => b, StringComparer.Ordinal).ToArray()
                    ?? Array.Empty<string>();
            }
        }

        /// <summary>BadgeService.GetAll()</summary>
        public IReadOnlyList<BadgeSpec> GetAll()
        {
            lock (_lock)
            {
                return _definitions.Values.ToArray();
            }
        }

        /// <summary>BadgeService.OnAward(Username, Callback) — receives the BadgeId.</summary>
        public void OnAward(string username, Action<string> callback)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                if (!_awardCallbacks.TryGetValue(username, out var list))
                    _awardCallbacks[username] = list = new List<Action<string>>();
                list.Add(callback);
            }
        }

        private void RequireDefinition(string badgeId)
        {
            RequireId(badgeId);
            if (!_definitions.ContainsKey(badgeId))
                throw new InvalidOperationException($"No badge '{badgeId}'.");
        }

        private static void RequireId(string badgeId)
        {
            if (string.IsNullOrWhiteSpace(badgeId))
                throw new ArgumentException("BadgeId must not be empty.", nameof(badgeId));
        }
    }

    /// <summary>
    /// QuestService registers multi-step quests, assigns them to players and
    /// tracks per-step progress; completing all steps completes the quest.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#questservice
    /// </summary>
    public class QuestService
    {
        /// <summary>A quest definition (steps count + reward table).</summary>
        public sealed record QuestSpec(string QuestId, string Name, string Description, int Steps,
            IReadOnlyDictionary<string, object?> Rewards);

        /// <summary>Per-player quest progress (GetProgress payload).</summary>
        public sealed record QuestProgress(string QuestId, int StepIndex, double Progress, bool Completed);

        private sealed class Assignment
        {
            public int StepIndex = 1;
            public double Progress;
            public bool Completed;
        }

        private readonly object _lock = new();
        private readonly Dictionary<string, QuestSpec> _definitions = new(StringComparer.Ordinal);
        private readonly Dictionary<string, Dictionary<string, Assignment>> _assignments = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action<string>>> _completeCallbacks = new(StringComparer.Ordinal);

        /// <summary>QuestService.Register(QuestId, Name, Description, Steps, Rewards)</summary>
        public void Register(string questId, string name, string description, int steps,
            IReadOnlyDictionary<string, object?>? rewards = null)
        {
            RequireId(questId);
            if (string.IsNullOrWhiteSpace(name))
                throw new ArgumentException("Name must not be empty.", nameof(name));
            if (steps < 1)
                throw new ArgumentException("Steps must be at least 1.", nameof(steps));

            lock (_lock)
            {
                if (!_definitions.TryAdd(questId,
                        new QuestSpec(questId, name, description, steps, rewards ?? new Dictionary<string, object?>())))
                    throw new InvalidOperationException($"Quest '{questId}' is already registered.");
            }
        }

        /// <summary>QuestService.Unregister(QuestId)</summary>
        public void Unregister(string questId)
        {
            RequireId(questId);
            lock (_lock)
            {
                if (!_definitions.Remove(questId))
                    throw new InvalidOperationException($"No quest '{questId}'.");
            }
        }

        /// <summary>QuestService.Assign(Username, QuestId)</summary>
        public void Assign(string username, string questId)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            var spec = RequireDefinition(questId);
            lock (_lock)
            {
                var quests = Assigned(username);
                if (!quests.TryAdd(questId, new Assignment()))
                    throw new InvalidOperationException($"Quest '{questId}' is already assigned to '{username}'.");
            }
        }

        /// <summary>QuestService.Unassign(Username, QuestId)</summary>
        public void Unassign(string username, string questId)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            RequireDefinition(questId);
            lock (_lock)
            {
                if (!Assigned(username).Remove(questId))
                    throw new InvalidOperationException($"Quest '{questId}' is not assigned to '{username}'.");
            }
        }

        /// <summary>QuestService.GetProgress(Username, QuestId)</summary>
        public QuestProgress? GetProgress(string username, string questId)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            RequireDefinition(questId);
            lock (_lock)
            {
                return Assigned(username).TryGetValue(questId, out var a)
                    ? new QuestProgress(questId, a.StepIndex, a.Progress, a.Completed)
                    : null;
            }
        }

        /// <summary>QuestService.AddProgress(Username, QuestId, StepIndex, Delta)</summary>
        public void AddProgress(string username, string questId, int stepIndex, double delta) =>
            MutateProgress(username, questId, stepIndex, p => p + delta);

        /// <summary>QuestService.SetProgress(Username, QuestId, StepIndex, Progress)</summary>
        public void SetProgress(string username, string questId, int stepIndex, double progress) =>
            MutateProgress(username, questId, stepIndex, _ => progress);

        /// <summary>QuestService.Complete(Username, QuestId)</summary>
        public void Complete(string username, string questId)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            RequireDefinition(questId);

            List<Action<string>>? callbacks = null;
            lock (_lock)
            {
                if (!Assigned(username).TryGetValue(questId, out var assignment))
                    throw new InvalidOperationException($"Quest '{questId}' is not assigned to '{username}'.");
                if (!assignment.Completed)
                {
                    assignment.Completed = true;
                    _completeCallbacks.TryGetValue(username, out callbacks);
                }
            }
            callbacks?.ForEach(cb => cb(questId));
        }

        /// <summary>QuestService.IsCompleted(Username, QuestId)</summary>
        public bool IsCompleted(string username, string questId)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            RequireDefinition(questId);
            lock (_lock)
            {
                return Assigned(username).TryGetValue(questId, out var a) && a.Completed;
            }
        }

        /// <summary>QuestService.GetActive(Username) — assigned quest ids that are not completed.</summary>
        public IReadOnlyList<string> GetActive(string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            lock (_lock)
            {
                return Assigned(username)
                    .Where(kv => !kv.Value.Completed)
                    .Select(kv => kv.Key)
                    .ToArray();
            }
        }

        /// <summary>QuestService.OnComplete(Username, Callback) — receives the QuestId.</summary>
        public void OnComplete(string username, Action<string> callback)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                if (!_completeCallbacks.TryGetValue(username, out var list))
                    _completeCallbacks[username] = list = new List<Action<string>>();
                list.Add(callback);
            }
        }

        private void MutateProgress(string username, string questId, int stepIndex, Func<double, double> change)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            var spec = RequireDefinition(questId);
            if (stepIndex < 1 || stepIndex > spec.Steps)
                throw new ArgumentException($"StepIndex must be between 1 and {spec.Steps}.", nameof(stepIndex));

            lock (_lock)
            {
                if (!Assigned(username).TryGetValue(questId, out var assignment))
                    throw new InvalidOperationException($"Quest '{questId}' is not assigned to '{username}'.");
                assignment.StepIndex = stepIndex;
                assignment.Progress = Math.Max(0, change(assignment.Progress));
            }
        }

        private Dictionary<string, Assignment> Assigned(string username)
        {
            if (!_assignments.TryGetValue(username, out var quests))
                _assignments[username] = quests = new Dictionary<string, Assignment>(StringComparer.Ordinal);
            return quests;
        }

        private QuestSpec RequireDefinition(string questId)
        {
            RequireId(questId);
            return _definitions.GetValueOrDefault(questId)
                ?? throw new InvalidOperationException($"No quest '{questId}'.");
        }

        private static void RequireId(string questId)
        {
            if (string.IsNullOrWhiteSpace(questId))
                throw new ArgumentException("QuestId must not be empty.", nameof(questId));
        }
    }

    /// <summary>
    /// RewardService manages daily check-in streaks, login rewards and
    /// redeem codes (with MaxUses and expiry). All day boundaries use UTC
    /// from the injectable clock.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#rewardservice
    /// </summary>
    public class RewardService
    {
        /// <summary>A registered redeem code (GetCodeInfo payload).</summary>
        public sealed record CodeInfo(string Code, IReadOnlyDictionary<string, object?> Rewards,
            int? MaxUses, DateTime? ExpiresAt, int TimesUsed);

        private readonly object _lock = new();
        private readonly Func<DateTime> _clock;
        private readonly Dictionary<string, DateTime> _lastDailyClaim = new(StringComparer.Ordinal);
        private readonly Dictionary<string, int> _streaks = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<string>> _loginRewardsClaimed = new(StringComparer.Ordinal);
        private readonly Dictionary<string, (IReadOnlyDictionary<string, object?> Rewards, int? MaxUses,
            DateTime? ExpiresAt, int TimesUsed)> _codes = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action<string>>> _claimCallbacks = new(StringComparer.Ordinal);

        public RewardService(Func<DateTime>? clock = null) => _clock = clock ?? (() => DateTime.UtcNow);

        /// <summary>RewardService.ClaimDaily(Username) — once per UTC day; keeps the streak.</summary>
        public int ClaimDaily(string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));

            lock (_lock)
            {
                DateTime today = _clock().Date;
                if (_lastDailyClaim.TryGetValue(username, out var last) && last == today)
                    throw new InvalidOperationException(
                        $"'{username}' already claimed the daily reward for {today:yyyy-MM-dd}.");

                DateTime yesterday = today.AddDays(-1);
                int streak = _lastDailyClaim.TryGetValue(username, out var previous) && previous == yesterday
                    ? _streaks.GetValueOrDefault(username) + 1
                    : 1;
                _streaks[username] = streak;
                _lastDailyClaim[username] = today;
                FireClaim(username, "daily:" + today.ToString("yyyy-MM-dd"));
                return streak;
            }
        }

        /// <summary>RewardService.GetDailyStreak(Username)</summary>
        public int GetDailyStreak(string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            lock (_lock)
            {
                return _streaks.GetValueOrDefault(username);
            }
        }

        /// <summary>
        /// RewardService.ClaimLoginReward(Username, RewardId) — once per UTC
        /// day per (username, rewardId).
        /// </summary>
        public void ClaimLoginReward(string username, string rewardId)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (string.IsNullOrWhiteSpace(rewardId))
                throw new ArgumentException("RewardId must not be empty.", nameof(rewardId));

            lock (_lock)
            {
                string marker = $"{_clock().Date:yyyy-MM-dd}\u0001{rewardId}";
                if (!_loginRewardsClaimed.TryGetValue(username, out var claimed))
                    _loginRewardsClaimed[username] = claimed = new List<string>();
                if (claimed.Contains(marker))
                    throw new InvalidOperationException(
                        $"'{username}' already claimed login reward '{rewardId}' today.");
                claimed.Add(marker);
            }
            FireClaim(username, rewardId);
        }

        /// <summary>RewardService.RegisterCode(Code, Rewards, MaxUses, ExpiresAt)</summary>
        public void RegisterCode(string code, IReadOnlyDictionary<string, object?> rewards,
            int? maxUses = null, DateTime? expiresAt = null)
        {
            RequireCode(code);
            if (rewards == null || rewards.Count == 0)
                throw new ArgumentException("A code needs at least one reward.", nameof(rewards));
            if (maxUses.HasValue && maxUses.Value < 1)
                throw new ArgumentException("MaxUses must be at least 1.", nameof(maxUses));

            lock (_lock)
            {
                if (!_codes.TryAdd(code, (rewards, maxUses, expiresAt, 0)))
                    throw new InvalidOperationException($"Code '{code}' is already registered.");
            }
        }

        /// <summary>RewardService.RedeemCode(Username, Code) — returns the rewards table.</summary>
        public IReadOnlyDictionary<string, object?> RedeemCode(string username, string code)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            RequireCode(code);

            lock (_lock)
            {
                if (!_codes.TryGetValue(code, out var info))
                    throw new InvalidOperationException($"No code '{code}'.");
                if (info.ExpiresAt.HasValue && _clock() > info.ExpiresAt.Value)
                    throw new InvalidOperationException($"Code '{code}' has expired.");
                if (info.MaxUses.HasValue && info.TimesUsed >= info.MaxUses.Value)
                    throw new InvalidOperationException($"Code '{code}' has no uses left.");

                string marker = code + "\u0001" + username;
                if (!_perUserRedeemed.Add(marker))
                    throw new InvalidOperationException($"'{username}' already redeemed '{code}'.");
                _codes[code] = info with { TimesUsed = info.TimesUsed + 1 };
                return info.Rewards;
            }
        }

        /// <summary>RewardService.RevokeCode(Code)</summary>
        public void RevokeCode(string code)
        {
            RequireCode(code);
            lock (_lock)
            {
                if (!_codes.Remove(code))
                    throw new InvalidOperationException($"No code '{code}'.");
            }
        }

        /// <summary>RewardService.GetCodeInfo(Code)</summary>
        public CodeInfo? GetCodeInfo(string code)
        {
            RequireCode(code);
            lock (_lock)
            {
                return _codes.TryGetValue(code, out var info)
                    ? new CodeInfo(code, info.Rewards, info.MaxUses, info.ExpiresAt, info.TimesUsed)
                    : null;
            }
        }

        /// <summary>RewardService.OnClaim(Username, Callback) — receives a claim marker.</summary>
        public void OnClaim(string username, Action<string> callback)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                if (!_claimCallbacks.TryGetValue(username, out var list))
                    _claimCallbacks[username] = list = new List<Action<string>>();
                list.Add(callback);
            }
        }

        private void FireClaim(string username, string marker)
        {
            Action<string>[] callbacks;
            lock (_lock)
            {
                callbacks = _claimCallbacks.TryGetValue(username, out var list)
                    ? list.ToArray()
                    : Array.Empty<Action<string>>();
            }
            foreach (var callback in callbacks)
                callback(marker);
        }

        private readonly HashSet<string> _perUserRedeemed = new(StringComparer.Ordinal);

        private static void RequireCode(string code)
        {
            if (string.IsNullOrWhiteSpace(code))
                throw new ArgumentException("Code must not be empty.", nameof(code));
        }
    }

    /// <summary>
    /// LeaderboardService manages named leaderboards with Ascending/
    /// Descending order, scores, ranks and resets.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#leaderboardservice
    /// </summary>
    public class LeaderboardService
    {
        /// <summary>A leaderboard entry (GetTopPlayers payload).</summary>
        public sealed record LeaderboardEntry(string Username, double Score);

        private sealed class Board
        {
            public string Order = "Descending";
            public Dictionary<string, double> Scores = new(StringComparer.Ordinal);
        }

        private readonly object _lock = new();
        private readonly Dictionary<string, Board> _boards = new(StringComparer.OrdinalIgnoreCase);

        /// <summary>LeaderboardService.CreateBoard(BoardName, Order)</summary>
        public void CreateBoard(string boardName, string order = "Descending")
        {
            RequireBoardName(boardName);
            string canonical = NormalizeOrder(order);
            lock (_lock)
            {
                if (!_boards.TryAdd(boardName, new Board { Order = canonical }))
                    throw new InvalidOperationException($"Leaderboard '{boardName}' already exists.");
            }
        }

        /// <summary>LeaderboardService.DeleteBoard(BoardName)</summary>
        public void DeleteBoard(string boardName)
        {
            RequireBoardName(boardName);
            lock (_lock)
            {
                if (!_boards.Remove(boardName))
                    throw new InvalidOperationException($"No leaderboard '{boardName}'.");
            }
        }

        /// <summary>LeaderboardService.SetScore(BoardName, Username, Score)</summary>
        public void SetScore(string boardName, string username, double score)
        {
            var board = RequireBoard(boardName);
            lock (_lock)
            {
                RequirePlayer(board, username);
                board.Scores[username] = score;
            }
        }

        /// <summary>LeaderboardService.GetScore(BoardName, Username) — null when the player has no score.</summary>
        public double? GetScore(string boardName, string username)
        {
            var board = RequireBoard(boardName);
            lock (_lock)
            {
                return board.Scores.TryGetValue(username, out var score) ? score : null;
            }
        }

        /// <summary>LeaderboardService.AddScore(BoardName, Username, Delta)</summary>
        public void AddScore(string boardName, string username, double delta)
        {
            var board = RequireBoard(boardName);
            lock (_lock)
            {
                RequirePlayer(board, username);
                board.Scores[username] = board.Scores.GetValueOrDefault(username) + delta;
            }
        }

        /// <summary>LeaderboardService.GetTopPlayers(BoardName, Limit)</summary>
        public IReadOnlyList<LeaderboardEntry> GetTopPlayers(string boardName, int limit = 10)
        {
            if (limit < 1)
                throw new ArgumentException("Limit must be at least 1.", nameof(limit));
            var board = RequireBoard(boardName);
            lock (_lock)
            {
                IEnumerable<KeyValuePair<string, double>> ordered = board.Order == "Ascending"
                    ? board.Scores.OrderBy(kv => kv.Value)
                    : board.Scores.OrderByDescending(kv => kv.Value);
                return ordered.Take(limit).Select(kv => new LeaderboardEntry(kv.Key, kv.Value)).ToArray();
            }
        }

        /// <summary>LeaderboardService.GetRank(BoardName, Username) — 1-based; null when unranked.</summary>
        public int? GetRank(string boardName, string username)
        {
            var board = RequireBoard(boardName);
            lock (_lock)
            {
                if (!board.Scores.ContainsKey(username))
                    return null;
                int rank = 1;
                foreach (var kv in board.Scores)
                {
                    if (kv.Key == username)
                        continue;
                    bool ahead = board.Order == "Ascending"
                        ? kv.Value < board.Scores[username]
                        : kv.Value > board.Scores[username];
                    if (ahead || (kv.Value == board.Scores[username] && string.CompareOrdinal(kv.Key, username) < 0))
                        rank++;
                }
                return rank;
            }
        }

        /// <summary>LeaderboardService.ResetBoard(BoardName) — clears scores, keeps the board.</summary>
        public void ResetBoard(string boardName)
        {
            var board = RequireBoard(boardName);
            lock (_lock)
            {
                board.Scores.Clear();
            }
        }

        private Board RequireBoard(string boardName)
        {
            if (string.IsNullOrWhiteSpace(boardName))
                throw new ArgumentException("BoardName must not be empty.", nameof(boardName));
            return _boards.GetValueOrDefault(boardName)
                ?? throw new InvalidOperationException($"No leaderboard '{boardName}'.");
        }

        private static void RequireBoardName(string boardName)
        {
            if (string.IsNullOrWhiteSpace(boardName))
                throw new ArgumentException("BoardName must not be empty.", nameof(boardName));
        }

        private static void RequirePlayer(Board board, string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
        }

        private static string NormalizeOrder(string order) =>
            string.Equals(order, "Ascending", StringComparison.OrdinalIgnoreCase) ? "Ascending"
            : string.Equals(order, "Descending", StringComparison.OrdinalIgnoreCase) ? "Descending"
            : throw new ArgumentException("Order must be \"Ascending\" or \"Descending\".", nameof(order));
    }

    /// <summary>
    /// CooldownService manages per-player, per-id cooldowns (skills, items,
    /// actions). OnReady callbacks fire lazily when the expiry is observed
    /// (the engine tick calls IsReady/GetRemaining).
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#cooldownservice
    /// </summary>
    public class CooldownService
    {
        private sealed class Entry
        {
            public DateTime EndsAtUtc;
        }

        private readonly object _lock = new();
        private readonly Func<DateTime> _clock;
        private readonly Dictionary<string, Entry> _entries = new(StringComparer.Ordinal); // "user\u0001id"
        private readonly Dictionary<string, List<Action>> _readyCallbacks = new(StringComparer.Ordinal);

        public CooldownService(Func<DateTime>? clock = null) => _clock = clock ?? (() => DateTime.UtcNow);

        /// <summary>CooldownService.Start(Username, CooldownId, Duration)</summary>
        public void Start(string username, string cooldownId, double duration)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (string.IsNullOrWhiteSpace(cooldownId))
                throw new ArgumentException("CooldownId must not be empty.", nameof(cooldownId));
            if (duration < 0)
                throw new ArgumentException("Duration must not be negative.", nameof(duration));

            lock (_lock)
            {
                _entries[Key(username, cooldownId)] = new Entry { EndsAtUtc = _clock().AddSeconds(duration) };
            }
        }

        /// <summary>CooldownService.Cancel(Username, CooldownId)</summary>
        public void Cancel(string username, string cooldownId) => Remove(username, cooldownId);

        /// <summary>CooldownService.Reset(Username, CooldownId) — clears the cooldown so it is ready again.</summary>
        public void Reset(string username, string cooldownId)
        {
            Remove(username, cooldownId);
            FireReady(username, cooldownId);
        }

        /// <summary>CooldownService.IsReady(Username, CooldownId)</summary>
        public bool IsReady(string username, string cooldownId)
        {
            Require(username, cooldownId);
            lock (_lock)
            {
                return !ActiveEnd(username, cooldownId).HasValue;
            }
        }

        /// <summary>CooldownService.GetRemaining(Username, CooldownId) — seconds left (0 when ready).</summary>
        public double GetRemaining(string username, string cooldownId)
        {
            Require(username, cooldownId);
            lock (_lock)
            {
                var endsAt = ActiveEnd(username, cooldownId);
                return endsAt.HasValue ? Math.Max(0, (endsAt.Value - _clock()).TotalSeconds) : 0;
            }
        }

        /// <summary>CooldownService.GetAll(Username) — remaining seconds per active cooldown.</summary>
        public IReadOnlyDictionary<string, double> GetAll(string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            lock (_lock)
            {
                var result = new Dictionary<string, double>(StringComparer.Ordinal);
                foreach (string key in _entries.Keys.Where(k => k.StartsWith(username + "\u0001", StringComparison.Ordinal)))
                {
                    string id = key[(username.Length + 1)..];
                    result[id] = GetRemaining(username, id);
                }
                return result;
            }
        }

        /// <summary>CooldownService.OnReady(Username, CooldownId, Callback)</summary>
        public void OnReady(string username, string cooldownId, Action callback)
        {
            Require(username, cooldownId);
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                string key = Key(username, cooldownId);
                if (!_readyCallbacks.TryGetValue(key, out var list))
                    _readyCallbacks[key] = list = new List<Action>();
                list.Add(callback);
            }
        }

        private DateTime? ActiveEnd(string username, string cooldownId)
        {
            string key = Key(username, cooldownId);
            if (!_entries.TryGetValue(key, out var entry))
                return null;

            if (_clock() >= entry.EndsAtUtc)
            {
                _entries.Remove(key);
                FireReady(username, cooldownId);
                return null;
            }
            return entry.EndsAtUtc;
        }

        private void Remove(string username, string cooldownId)
        {
            Require(username, cooldownId);
            lock (_lock)
            {
                _entries.Remove(Key(username, cooldownId));
            }
        }

        private void FireReady(string username, string cooldownId)
        {
            List<Action>? callbacks;
            lock (_lock)
            {
                _readyCallbacks.TryGetValue(Key(username, cooldownId), out callbacks);
            }
            callbacks?.ForEach(cb => cb());
        }

        private static string Key(string username, string cooldownId) => username + "\u0001" + cooldownId;

        private static void Require(string username, string cooldownId)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (string.IsNullOrWhiteSpace(cooldownId))
                throw new ArgumentException("CooldownId must not be empty.", nameof(cooldownId));
        }
    }
}
