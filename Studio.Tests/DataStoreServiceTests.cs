using System;
using System.Collections.Generic;
using System.Linq;
using Noobietoria.Studio.Core;
using Xunit;

namespace Noobietoria.Studio.Tests
{
    public class DataStoreServiceTests
    {
        private static readonly DateTime FixedNow = new(2026, 1, 1, 12, 0, 0, DateTimeKind.Utc);

        private sealed class MutableClock
        {
            private DateTime _now;

            public MutableClock(DateTime start) => _now = start;

            public DateTime Next() => _now;

            public void Advance(TimeSpan by) => _now = _now.Add(by);
        }

        private static DataStoreService NewService(out MutableClock clock)
        {
            clock = new MutableClock(FixedNow);
            return new DataStoreService(clock.Next);
        }

        // ---- session lifecycle ----

        [Fact]
        public void Init_InsertGetDelete_RoundTrips()
        {
            var service = NewService(out _);

            service.Init("ABCDEFGHIJKLMNOP");
            service.Insert("ABCDEFGHIJKLMNOP", "coins:alice", "{\"amount\": 10}");

            Assert.Equal("{\"amount\": 10}", service.Get("ABCDEFGHIJKLMNOP", "coins:alice"));
            Assert.Contains("coins:alice", service.GetKeys("ABCDEFGHIJKLMNOP"));

            Assert.True(service.Delete("ABCDEFGHIJKLMNOP", "coins:alice"));
            Assert.Null(service.Get("ABCDEFGHIJKLMNOP", "coins:alice"));
            Assert.False(service.Delete("ABCDEFGHIJKLMNOP", "coins:alice"));
        }

        [Fact]
        public void Init_RejectsNonBase32SessionIds()
        {
            var service = NewService(out _);

            Assert.Throws<ArgumentException>(() => service.Init("has space!"));
            Assert.Throws<ArgumentException>(() => service.Init(""));
            // 0, 1, 8, 9 are not part of the base32 alphabet.
            Assert.Throws<ArgumentException>(() => service.Init("01234567"));
        }

        [Fact]
        public void Init_AcceptsLowercaseAndPadding()
        {
            var service = NewService(out _);

            service.Init("abcdefgh==");

            Assert.True(service.IsSessionOpen("abcdefgh=="));
        }

        [Fact]
        public void Init_SameSessionId_Twice_Throws()
        {
            var service = NewService(out _);

            service.Init("ABCDEFGHIJKLMNOP");

            Assert.Throws<InvalidOperationException>(() => service.Init("ABCDEFGHIJKLMNOP"));
        }

        [Fact]
        public void Close_EndsSession_AndSubsequentOperationsThrow()
        {
            var service = NewService(out _);
            service.Init("ABCDEFGHIJKLMNOP");

            service.Close("ABCDEFGHIJKLMNOP");

            Assert.False(service.IsSessionOpen("ABCDEFGHIJKLMNOP"));
            Assert.Throws<InvalidOperationException>(() =>
                service.Get("ABCDEFGHIJKLMNOP", "key"));
            Assert.Throws<InvalidOperationException>(() => service.Close("ABCDEFGHIJKLMNOP"));
        }

        [Fact]
        public void Operations_WithoutSession_Throw()
        {
            var service = NewService(out _);

            Assert.Throws<InvalidOperationException>(() => service.Get("ABCDEFGHIJKLMNOP", "key"));
            Assert.Throws<InvalidOperationException>(() =>
                service.Insert("ABCDEFGHIJKLMNOP", "key", "1"));
            Assert.Throws<InvalidOperationException>(() => service.GetKeys("ABCDEFGHIJKLMNOP"));
            Assert.Throws<InvalidOperationException>(() => service.Delete("ABCDEFGHIJKLMNOP", "key"));
        }

        [Fact]
        public void Init_LimitsConcurrentConnectionsTo16()
        {
            var service = NewService(out _);
            string alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

            for (int i = 0; i < 16; i++)
                service.Init(new string('A', 15) + alphabet[i]);

            Assert.Throws<InvalidOperationException>(() =>
                service.Init(new string('A', 15) + alphabet[16]));
        }

        [Fact]
        public void IdleSession_AutoClosesAfter15Seconds()
        {
            var service = NewService(out var clock);
            service.Init("ABCDEFGHIJKLMNOP");

            clock.Advance(TimeSpan.FromSeconds(15));

            Assert.False(service.IsSessionOpen("ABCDEFGHIJKLMNOP"));
            Assert.Throws<InvalidOperationException>(() => service.Get("ABCDEFGHIJKLMNOP", "key"));
        }

        [Fact]
        public void Operations_RefreshIdleTimeout()
        {
            var service = NewService(out var clock);
            service.Init("ABCDEFGHIJKLMNOP");

            clock.Advance(TimeSpan.FromSeconds(10));
            service.Insert("ABCDEFGHIJKLMNOP", "key", "1");   // activity refreshes the timer

            clock.Advance(TimeSpan.FromSeconds(14));           // 14s after the refresh
            Assert.Equal("1", service.Get("ABCDEFGHIJKLMNOP", "key"));

            clock.Advance(TimeSpan.FromSeconds(16));           // 16s after the last activity
            Assert.Throws<InvalidOperationException>(() =>
                service.Get("ABCDEFGHIJKLMNOP", "key"));
        }

        // ---- validation limits ----

        [Fact]
        public void Key_MustBeNonEmptyAscii_AndAtMost50Characters()
        {
            var service = NewService(out _);
            service.Init("ABCDEFGHIJKLMNOP");

            Assert.Throws<ArgumentException>(() =>
                service.Insert("ABCDEFGHIJKLMNOP", "", "1"));
            Assert.Throws<ArgumentException>(() =>
                service.Insert("ABCDEFGHIJKLMNOP", new string('k', 51), "1"));
            Assert.Throws<ArgumentException>(() =>
                service.Insert("ABCDEFGHIJKLMNOP", "caf\u00E9", "1")); // non-ASCII

            service.Insert("ABCDEFGHIJKLMNOP", new string('k', 50), "1");
            Assert.Equal("1", service.Get("ABCDEFGHIJKLMNOP", new string('k', 50)));
        }

        [Fact]
        public void Value_MustBeValidJson()
        {
            var service = NewService(out _);
            service.Init("ABCDEFGHIJKLMNOP");

            Assert.Throws<ArgumentException>(() =>
                service.Insert("ABCDEFGHIJKLMNOP", "key", "not json"));

            service.Insert("ABCDEFGHIJKLMNOP", "text", "\"hello\"");
            service.Insert("ABCDEFGHIJKLMNOP", "number", "42");
            service.Insert("ABCDEFGHIJKLMNOP", "object", "{\"a\":[1,2]}");

            Assert.Equal("\"hello\"", service.Get("ABCDEFGHIJKLMNOP", "text"));
            Assert.Equal("42", service.Get("ABCDEFGHIJKLMNOP", "number"));
        }

        [Fact]
        public void Value_IsCappedAtOneMegabyte()
        {
            var service = NewService(out _);
            service.Init("ABCDEFGHIJKLMNOP");

            string maxPayload = "\"" + new string('a', (int)DataStoreService.MaxValueBytes - 2) + "\"";
            service.Insert("ABCDEFGHIJKLMNOP", "big", maxPayload);

            string tooBig = "\"" + new string('a', (int)DataStoreService.MaxValueBytes - 1) + "\"";
            Assert.Throws<ArgumentException>(() =>
                service.Insert("ABCDEFGHIJKLMNOP", "bigger", tooBig));
        }

        [Fact]
        public void Insert_OverwritesExistingKey_AndCapacityIsAccountedFor()
        {
            var service = NewService(out _);
            service.TotalCapacityBytes = 12;
            service.Init("ABCDEFGHIJKLMNOP");

            service.Insert("ABCDEFGHIJKLMNOP", "key", "\"12345\"");   // 7 bytes
            service.Insert("ABCDEFGHIJKLMNOP", "key", "\"54321\"");   // overwrite, still 7 bytes

            Assert.Equal("\"54321\"", service.Get("ABCDEFGHIJKLMNOP", "key"));

            service.Insert("ABCDEFGHIJKLMNOP", "other", "\"12\"");    // 4 bytes -> 11 total
            Assert.Throws<InvalidOperationException>(() =>
                service.Insert("ABCDEFGHIJKLMNOP", "third", "\"1\"")); // would be 14
        }

        // ---- request budget ----

        [Fact]
        public void RequestBudget_Exhausts_AndResetsNextWindow()
        {
            var service = NewService(out var clock);
            service.Init("ABCDEFGHIJKLMNOP");
            service.Insert("ABCDEFGHIJKLMNOP", "key", "1"); // 1 request

            for (int i = 1; i < 15; i++)                     // 14 more -> 15 total
                service.Get("ABCDEFGHIJKLMNOP", "key");

            Assert.Equal(0, service.GetRequestBudgetForRequestType(DataStoreRequestType.GetAsync));
            Assert.Throws<InvalidOperationException>(() =>
                service.Get("ABCDEFGHIJKLMNOP", "key"));

            // A full minute passes; the session idles out with it, so reopen.
            clock.Advance(TimeSpan.FromMinutes(1));
            service.Init("ABCDEFGHIJKLMNOP");

            Assert.Equal(15, service.GetRequestBudgetForRequestType(DataStoreRequestType.GetAsync));
            Assert.Equal("1", service.Get("ABCDEFGHIJKLMNOP", "key"));
        }

        [Fact]
        public void RequestBudget_ScalesWithConcurrentUsers()
        {
            var service = NewService(out _);
            service.Init("ABCDEFGHIJKLMNOP");
            service.ConcurrentUsers = 3; // 15 + 10 x 3 = 45

            Assert.Equal(45, service.GetRequestBudgetForRequestType(DataStoreRequestType.SetIncrementAsync));
        }

        // ---- Roblox-side APIs ----

        [Fact]
        public void GetDataStore_CompatRoundTrip()
        {
            var service = NewService(out _);
            var store = service.GetDataStore("Coins", "player");

            store.SetAsync("alice", "{\"amount\": 100}");
            Assert.Equal("{\"amount\": 100}", store.GetAsync("alice"));
            Assert.Null(store.GetAsync("bob"));

            store.UpdateAsync("alice", current =>
            {
                Assert.Equal("{\"amount\": 100}", current);
                return "{\"amount\": 150}";
            });
            Assert.Equal("{\"amount\": 150}", store.GetAsync("alice"));

            Assert.True(store.RemoveAsync("alice"));
            Assert.Null(store.GetAsync("alice"));

            store.UpdateAsync("fresh", current =>
            {
                Assert.Null(current);
                return "\"created\"";
            });
            Assert.Equal("\"created\"", store.GetAsync("fresh"));

            store.UpdateAsync("fresh", _ => null); // returning null deletes
            Assert.Null(store.GetAsync("fresh"));
        }

        [Fact]
        public void GetDataStore_LimitsTo15DistinctNames()
        {
            var service = NewService(out _);

            for (int i = 0; i < 15; i++)
                service.GetDataStore("store" + i);

            Assert.Throws<InvalidOperationException>(() => service.GetDataStore("store15"));

            service.GetDataStore("store0"); // known names don't double-count
        }

        [Fact]
        public void GetOrderedDataStore_GetSortedAsync_SortsNumericValues()
        {
            var service = NewService(out _);
            var store = service.GetOrderedDataStore("Leaderboard");

            store.SetAsync("alice", "30");
            store.SetAsync("bob", "10");
            store.SetAsync("carol", "20");
            store.SetAsync("tag", "\"not a number\"");

            var ascending = store.GetSortedAsync(ascending: true, limit: 10);
            Assert.Equal(new[] { "bob", "carol", "alice" }, ascending.Select(kv => kv.Key).ToArray());

            var descending = store.GetSortedAsync(ascending: false, limit: 2);
            Assert.Equal(new[] { "alice", "carol" }, descending.Select(kv => kv.Key).ToArray());

            var filtered = store.GetSortedAsync(ascending: true, limit: 10, minValue: 15);
            Assert.Equal(new[] { "carol", "alice" }, filtered.Select(kv => kv.Key).ToArray());
        }
    }
}
