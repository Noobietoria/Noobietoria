using System;
using System.Linq;
using Noobietoria.Studio.Core;
using Xunit;

namespace Noobietoria.Studio.Tests
{
    public class AccessoryServiceTests
    {
        [Fact]
        public void Equip_ParsesCommaSeparatedIds()
        {
            var service = new AccessoryService();
            service.Equip("Player1", "Player", " 1082345 , 1082350, 1082360");
            service.Equip("ShopKeeper", "NPC", " 12345678 , 87654321");

            Assert.Equal(new[] { "1082345", "1082350", "1082360" }, service.GetEquipped("Player1"));
            Assert.Equal(new[] { "12345678", "87654321" }, service.GetEquipped("ShopKeeper"));
            Assert.Empty(service.GetEquipped("Nobody"));

            Assert.Throws<ArgumentException>(() => service.Equip("", "Player", "1"));
            Assert.Throws<ArgumentException>(() => service.Equip("Player1", "Pet", "1"));
            Assert.Throws<ArgumentException>(() => service.Equip("Player1", "Player", " , , "));
        }
    }

    public class AnalyticsServiceTests
    {
        [Fact]
        public void Events_Errors_Counters()
        {
            var now = new DateTime(2026, 1, 1, 10, 0, 0, DateTimeKind.Utc);
            var service = new AnalyticsService(() => now);

            service.TrackEvent("level_start", "Alice", new System.Collections.Generic.Dictionary<string, object?> { ["level"] = 1 });
            service.TrackEvent("level_start");
            service.TrackError("null reference", "Bob");

            Assert.Equal(2, service.GetEvents("level_start").Count);
            Assert.Single(service.GetEvents("level_start", 1));
            Assert.Single(service.GetEvents("null reference"), e => e.Username == "Bob");

            service.ClearEvents("level_start");
            Assert.Empty(service.GetEvents("level_start"));

            service.SetCounter("plays", 10);
            service.IncrementCounter("plays");
            service.IncrementCounter("plays", 4);
            Assert.Equal(15, service.GetCounter("plays"));
            Assert.Equal(0, service.GetCounter("unknown"));
        }
    }

    public class LocalizationServiceTests
    {
        [Fact]
        public void Register_Translate_PerPlayerLocale()
        {
            var service = new LocalizationService();
            service.Register("greeting", new System.Collections.Generic.Dictionary<string, string>
            {
                ["en"] = "Hello",
                ["vi"] = "Xin chào",
                ["ja"] = "こんにちは",
            });

            Assert.Equal("Xin chào", service.Translate("greeting", "vi"));
            Assert.Equal("Hello", service.TranslateFor("greeting", "Bob")); // default locale "en"
            Assert.Null(service.Translate("greeting", "de"));

            service.SetLocale("Alice", "vi");
            Assert.Equal("vi", service.GetLocale("Alice"));
            Assert.Equal("Xin chào", service.TranslateFor("greeting", "Alice"));

            Assert.Equal(new[] { "en", "ja", "vi" }, service.GetSupportedLocales());

            service.ImportTable(new System.Collections.Generic.Dictionary<string, System.Collections.Generic.IReadOnlyDictionary<string, string>>
            {
                ["greeting"] = new System.Collections.Generic.Dictionary<string, string> { ["en"] = "Hello!", ["de"] = "Hallo" },
            });
            Assert.Equal("Hallo", service.Translate("greeting", "de"));

            service.Unregister("greeting");
            Assert.Null(service.Translate("greeting", "en"));
            Assert.Throws<InvalidOperationException>(() => service.Unregister("greeting"));
        }
    }

    public class PassServiceTests
    {
        [Fact]
        public void Grant_Permissions_Revoke()
        {
            var service = new PassService();
            var grants = new System.Collections.Generic.List<string>();
            service.OnGrant("Alice", id => grants.Add(id));

            service.Register("vip", "VIP", "Very important person",
                new System.Collections.Generic.Dictionary<string, string> { ["fly"] = "Allowed", ["vip_zone"] = "Allowed" });
            service.Register("builder", "Builder", "Build rights",
                new System.Collections.Generic.Dictionary<string, string> { ["build"] = "Allowed" });

            service.Grant("Alice", "vip");
            service.Grant("Alice", "vip"); // idempotent
            Assert.True(service.HasPass("Alice", "vip"));
            Assert.Single(grants);

            service.Grant("Alice", "builder");
            Assert.True(service.CheckPermission("Alice", "fly"));
            Assert.True(service.CheckPermission("Alice", "build"));
            Assert.False(service.CheckPermission("Alice", "admin"));

            service.Revoke("Alice", "vip");
            Assert.False(service.HasPass("Alice", "vip"));
            Assert.False(service.CheckPermission("Alice", "fly"));
            Assert.Equal("builder", service.GetPasses("Alice").Single());
            Assert.Equal(2, service.GetAll().Count);
        }
    }

    public class AudioCServiceTests
    {
        [Fact]
        public void New_Play_Pause_Resume_Volume_Loop_Change()
        {
            var service = new AudioCService();
            service.New("BGM", "audio-42");
            Assert.Throws<InvalidOperationException>(() => service.New("BGM", "audio-42"));

            service.Resume("BGM");
            Assert.Equal(AudioCService.AudioState.Playing, service.Get("BGM")!.State);
            Assert.Throws<InvalidOperationException>(() => service.Resume("BGM"));

            service.Pause("BGM");
            Assert.Equal(AudioCService.AudioState.Paused, service.Get("BGM")!.State);

            service.SetVolume("BGM", 0.5);
            service.Loop("BGM", true);
            service.Change("BGM", "audio-99");

            var state = service.Get("BGM")!;
            Assert.Equal(0.5, state.Volume);
            Assert.True(state.Loop);
            Assert.Equal("audio-99", state.Id);

            service.Stop("BGM");
            Assert.Equal(AudioCService.AudioState.Stopped, service.Get("BGM")!.State);

            Assert.Throws<ArgumentException>(() => service.SetVolume("BGM", 2));
            Assert.Throws<InvalidOperationException>(() => service.SetVolume("Missing", 0.5));
        }
    }
}
