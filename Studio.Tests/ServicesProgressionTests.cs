using System;
using System.Linq;
using Noobietoria.Studio.Core;
using Xunit;

namespace Noobietoria.Studio.Tests
{
    public class AchievementServiceTests
    {
        [Fact]
        public void Register_Progress_AutoUnlock()
        {
            var service = new AchievementService();
            var unlocks = new System.Collections.Generic.List<string>();
            service.OnUnlock("Alice", id => unlocks.Add(id));

            service.Register("kill100", "Centurion", "Defeat 100 enemies", 100);
            service.SetProgress("Alice", "kill100", 50);
            Assert.Equal(50, service.GetProgress("Alice", "kill100"));
            Assert.False(service.IsUnlocked("Alice", "kill100"));

            service.AddProgress("Alice", "kill100", 50); // reaches MaxProgress -> auto unlock
            Assert.True(service.IsUnlocked("Alice", "kill100"));
            Assert.Equal(100, service.GetProgress("Alice", "kill100"));
            Assert.Equal("kill100", unlocks.Single());

            service.Unlock("Alice", "kill100"); // idempotent
            Assert.Single(unlocks);
            Assert.Contains("kill100", service.GetUnlocked("Alice"));

            Assert.Throws<ArgumentException>(() => service.SetProgress("Alice", "kill100", 101));
            Assert.Throws<InvalidOperationException>(() => service.GetProgress("Alice", "missing"));
        }
    }

    public class BadgeServiceTests
    {
        [Fact]
        public void Register_Award_Revoke()
        {
            var service = new BadgeService();
            var awards = new System.Collections.Generic.List<string>();
            service.OnAward("Alice", id => awards.Add(id));

            service.Register("founder", "Founder", "Early supporter");
            service.Award("Alice", "founder");
            service.Award("Alice", "founder"); // idempotent

            Assert.True(service.HasBadge("Alice", "founder"));
            Assert.Single(awards);
            Assert.Contains("founder", service.GetBadges("Alice"));

            service.Revoke("Alice", "founder");
            Assert.False(service.HasBadge("Alice", "founder"));
            Assert.Throws<InvalidOperationException>(() => service.Award("Alice", "missing"));
        }
    }

    public class QuestServiceTests
    {
        [Fact]
        public void QuestLifecycle_Steps_AndComplete()
        {
            var service = new QuestService();
            var completions = new System.Collections.Generic.List<string>();
            service.OnComplete("Alice", id => completions.Add(id));

            service.Register("tutorial", "Tutorial", "Learn the ropes", 3,
                new System.Collections.Generic.Dictionary<string, object?> { ["Coin"] = 100 });
            service.Assign("Alice", "tutorial");
            Assert.Throws<InvalidOperationException>(() => service.Assign("Alice", "tutorial"));

            service.AddProgress("Alice", "tutorial", 1, 5);
            service.SetProgress("Alice", "tutorial", 2, 10);
            var progress = service.GetProgress("Alice", "tutorial")!;
            Assert.Equal(2, progress.StepIndex);
            Assert.Equal(10, progress.Progress);

            Assert.Equal(new[] { "tutorial" }, service.GetActive("Alice"));
            service.Complete("Alice", "tutorial");
            Assert.True(service.IsCompleted("Alice", "tutorial"));
            Assert.Empty(service.GetActive("Alice"));
            Assert.Equal("tutorial", completions.Single());

            Assert.Throws<ArgumentException>(() => service.SetProgress("Alice", "tutorial", 9, 1));
            Assert.Throws<InvalidOperationException>(() => service.Complete("Bob", "tutorial"));
        }
    }

    public class RewardServiceTests
    {
        [Fact]
        public void DailyStreak_OncePerDay()
        {
            var now = new DateTime(2026, 1, 1, 10, 0, 0, DateTimeKind.Utc);
            var service = new RewardService(() => now);

            Assert.Equal(1, service.ClaimDaily("Alice"));
            Assert.Throws<InvalidOperationException>(() => service.ClaimDaily("Alice"));

            now = now.AddDays(1);
            Assert.Equal(2, service.ClaimDaily("Alice")); // consecutive day
            now = now.AddDays(3); // streak broken
            Assert.Equal(1, service.ClaimDaily("Alice"));
            Assert.Equal(1, service.GetDailyStreak("Alice"));
        }

        [Fact]
        public void LoginReward_OncePerDayPerReward()
        {
            var now = new DateTime(2026, 1, 1, 10, 0, 0, DateTimeKind.Utc);
            var service = new RewardService(() => now);

            service.ClaimLoginReward("Alice", "login-gift");
            Assert.Throws<InvalidOperationException>(() => service.ClaimLoginReward("Alice", "login-gift"));

            now = now.AddDays(1);
            service.ClaimLoginReward("Alice", "login-gift"); // next day works
        }

        [Fact]
        public void Codes_MaxUses_Expiry_Revoke()
        {
            var now = new DateTime(2026, 1, 1, 10, 0, 0, DateTimeKind.Utc);
            var service = new RewardService(() => now);
            var rewards = new System.Collections.Generic.Dictionary<string, object?> { ["Coin"] = 100.0 };

            service.RegisterCode("NEWBIE", rewards, maxUses: 2);
            var claimed = service.RedeemCode("Alice", "NEWBIE");
            Assert.Equal(100.0, claimed["Coin"]);
            service.RedeemCode("Bob", "NEWBIE");
            Assert.Throws<InvalidOperationException>(() => service.RedeemCode("Alice", "NEWBIE")); // per-user once
            Assert.Equal(2, service.GetCodeInfo("NEWBIE")!.TimesUsed);
            Assert.Throws<InvalidOperationException>(() => service.RedeemCode("Carol", "NEWBIE")); // out of uses

            service.RegisterCode("EXPIRED", rewards, expiresAt: now.AddMinutes(-1));
            Assert.Throws<InvalidOperationException>(() => service.RedeemCode("Alice", "EXPIRED"));

            service.RevokeCode("NEWBIE");
            Assert.Null(service.GetCodeInfo("NEWBIE"));
        }
    }

    public class LeaderboardServiceTests
    {
        [Fact]
        public void Scores_Ranks_TopPlayers_Reset()
        {
            var service = new LeaderboardService();
            service.CreateBoard("Wins", "Descending");
            service.CreateBoard("Fastest", "Ascending");
            Assert.Throws<InvalidOperationException>(() => service.CreateBoard("Wins"));

            service.SetScore("Wins", "Alice", 10);
            service.SetScore("Wins", "Bob", 30);
            service.AddScore("Wins", "Alice", 25); // 35, takes the lead

            Assert.Equal(35, service.GetScore("Wins", "Alice"));
            Assert.Equal(1, service.GetRank("Wins", "Alice"));
            Assert.Equal(2, service.GetRank("Wins", "Bob"));
            Assert.Equal(new[] { "Alice", "Bob" }, service.GetTopPlayers("Wins").Select(e => e.Username));

            service.SetScore("Fastest", "Alice", 12);
            service.SetScore("Fastest", "Bob", 8);
            Assert.Equal("Bob", service.GetTopPlayers("Fastest").First().Username); // ascending

            service.ResetBoard("Wins");
            Assert.Null(service.GetScore("Wins", "Alice"));
            Assert.Null(service.GetRank("Wins", "Bob"));
        }
    }

    public class CooldownServiceTests
    {
        [Fact]
        public void Start_Remaining_Ready_Callbacks()
        {
            var now = new DateTime(2026, 1, 1, 10, 0, 0, DateTimeKind.Utc);
            var service = new CooldownService(() => now);
            int ready = 0;
            service.OnReady("Alice", "Skill_Fireball", () => ready++);

            service.Start("Alice", "Skill_Fireball", 10);
            Assert.False(service.IsReady("Alice", "Skill_Fireball"));
            Assert.Equal(10, service.GetRemaining("Alice", "Skill_Fireball"));
            Assert.Equal(10, service.GetAll("Alice")["Skill_Fireball"]);

            now = now.AddSeconds(6);
            Assert.Equal(4, service.GetRemaining("Alice", "Skill_Fireball"));

            now = now.AddSeconds(4);
            Assert.True(service.IsReady("Alice", "Skill_Fireball")); // expiry observed
            Assert.Equal(1, ready); // OnReady fired on expiry
            Assert.Empty(service.GetAll("Alice"));

            service.Start("Alice", "Skill_Fireball", 10);
            service.Reset("Alice", "Skill_Fireball");
            Assert.True(service.IsReady("Alice", "Skill_Fireball"));
        }
    }
}
