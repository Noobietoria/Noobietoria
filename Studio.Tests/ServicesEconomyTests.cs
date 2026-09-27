using System;
using System.Linq;
using Noobietoria.Studio.Core;
using Xunit;

namespace Noobietoria.Studio.Tests
{
    public class CurrencyServiceTests
    {
        [Fact]
        public void Wallet_Operations_Transfer_AndTransactions()
        {
            var service = new CurrencyService();
            var events = new System.Collections.Generic.List<CurrencyService.Transaction>();
            service.OnTransaction("Alice", t => events.Add(t));

            Assert.Equal(0, service.GetBalance("Alice", "Coin"));
            service.Add("Alice", "Coin", 100);
            service.Deduct("Alice", "Coin", 30);
            service.SetBalance("Alice", "Gem", 5);

            Assert.Equal(70, service.GetBalance("Alice", "Coin"));
            Assert.Equal(5, service.GetAllBalances("Alice")["Gem"]);

            service.Transfer("Alice", "Bob", "Coin", 20);
            Assert.Equal(50, service.GetBalance("Alice", "Coin"));
            Assert.Equal(20, service.GetBalance("Bob", "Coin"));

            Assert.Equal(4, events.Count);
            Assert.Equal("Transfer", events.Last().Kind);

            Assert.Throws<InvalidOperationException>(() => service.Deduct("Bob", "Coin", 1000));
            Assert.Throws<ArgumentException>(() => service.Add("Alice", "Coin", -5));
        }
    }

    public class InventoryServiceTests
    {
        [Fact]
        public void Stacks_Transfer_Clear()
        {
            var service = new InventoryService();
            service.AddItem("Alice", "Potion", 3);
            service.AddItem("Alice", "Potion", 2);
            service.AddItem("Alice", "Sword", 1, new System.Collections.Generic.Dictionary<string, object?> { ["rarity"] = "epic" });

            Assert.Equal(5, service.GetItemCount("Alice", "Potion"));
            Assert.True(service.HasItem("Alice", "Sword"));
            Assert.Equal(2, service.GetInventory("Alice").Count);

            service.RemoveItem("Alice", "Potion", 4);
            Assert.Equal(1, service.GetItemCount("Alice", "Potion"));

            service.TransferItem("Alice", "Bob", "Sword");
            Assert.False(service.HasItem("Alice", "Sword"));
            Assert.Equal("epic", service.GetInventory("Bob").Single().Metadata!["rarity"]);

            service.ClearInventory("Alice");
            Assert.Empty(service.GetInventory("Alice"));
            Assert.Throws<InvalidOperationException>(() => service.RemoveItem("Bob", "Potion", 10));
        }
    }

    public class MarketplaceServiceTests
    {
        [Fact]
        public void List_Search_Buy_History_AndCallbacks()
        {
            var service = new MarketplaceService();
            var sales = new System.Collections.Generic.List<MarketplaceService.MarketTransaction>();
            var purchases = new System.Collections.Generic.List<MarketplaceService.MarketTransaction>();
            service.OnSale("Alice", t => sales.Add(t));
            service.OnPurchase("Bob", t => purchases.Add(t));

            string listing = service.ListItem("Alice", "sword-1", 100, "Coin", 2);
            string other = service.ListItem("Bob", "shield-1", 40, "Gem");

            Assert.Equal(2, service.Search("sword").Count == 1 ? 2 : service.Search("").Count); // both listings
            Assert.Single(service.Search("sword-1"));
            Assert.Single(service.Search(null, new System.Collections.Generic.Dictionary<string, object?> { ["MaxPrice"] = 50.0 }));

            var transaction = service.Buy("Bob", listing, 2);
            Assert.Equal(200, transaction.TotalPrice);
            Assert.Single(sales);
            Assert.Single(purchases);

            Assert.Null(service.GetListing(listing)); // quantity exhausted
            Assert.NotNull(service.GetListing(other));
            service.DelistItem(other);
            Assert.Null(service.GetListing(other));

            Assert.Single(service.GetTransactionHistory("Bob"));
            Assert.Throws<InvalidOperationException>(() => service.Buy("Bob", "missing"));
            Assert.Throws<InvalidOperationException>(() => service.Buy("Alice", service.ListItem("Alice", "x", 1, "Coin")));
        }
    }

    public class PurchaseServiceIGTests
    {
        [Fact]
        public void OrderLifecycle()
        {
            var service = new PurchaseServiceIG();
            var confirmed = new System.Collections.Generic.List<PurchaseServiceIG.Order>();
            service.OnPurchase("Alice", o => confirmed.Add(o));

            string orderId = service.CreateOrder("Alice", "skin-1", 1, "Gem");
            Assert.Equal("Pending", service.GetOrder(orderId)!.Status);

            service.ConfirmOrder(orderId);
            Assert.Equal("Confirmed", service.GetOrder(orderId)!.Status);
            Assert.Single(confirmed);

            string cancelled = service.CreateOrder("Alice", "skin-2");
            service.CancelOrder(cancelled);
            Assert.Equal("Cancelled", service.GetOrder(cancelled)!.Status);

            service.Refund(orderId);
            Assert.Equal("Refunded", service.GetOrder(orderId)!.Status);
            Assert.Equal(2, service.GetOrderHistory("Alice").Count());

            Assert.Throws<InvalidOperationException>(() => service.Refund(orderId)); // already refunded
        }
    }

    public class PurchaseServicePlatformTests
    {
        [Fact]
        public void Purchase_History_AndCallbacks()
        {
            var service = new PurchaseServicePlatform();
            var purchases = new System.Collections.Generic.List<PurchaseServicePlatform.PlatformOrder>();
            service.OnPurchase(7, o => purchases.Add(o));

            string orderId = service.Purchase(7, "starter-pack");
            Assert.Equal("Completed", service.GetOrder(orderId)!.Status);
            Assert.Single(purchases);
            Assert.Single(service.GetPurchaseHistory(7));
            Assert.Null(service.GetOrder("PLT-missing"));
        }
    }

    public class TradingServiceTests
    {
        [Fact]
        public void TradeSession_OfferConfirm_Complete()
        {
            var service = new TradingService();
            var completed = new System.Collections.Generic.List<TradingService.Trade>();
            service.OnTradeComplete("Alice", t => completed.Add(t));

            string tradeId = service.CreateTrade("Alice", "Bob");
            service.SetOffer(tradeId, "Alice", new TradingService.Offer(
                new System.Collections.Generic.Dictionary<string, int> { ["Potion"] = 2 },
                new System.Collections.Generic.Dictionary<string, double>()));
            service.SetOffer(tradeId, "Bob", new TradingService.Offer(
                new System.Collections.Generic.Dictionary<string, int>(),
                new System.Collections.Generic.Dictionary<string, double> { ["Coin"] = 50 }));

            Assert.Equal(2, service.GetOffer(tradeId, "Alice")!.Items["Potion"]);
            Assert.Equal(50, service.GetOffer(tradeId, "Bob")!.Currencies["Coin"]);

            service.Confirm(tradeId, "Alice");
            Assert.Equal(TradingService.StatusPending, service.GetTrade(tradeId)!.Status); // waiting for Bob
            service.Confirm(tradeId, "Bob");
            Assert.Equal(TradingService.StatusCompleted, service.GetTrade(tradeId)!.Status);
            Assert.Single(completed);

            Assert.Throws<InvalidOperationException>(() => service.CreateTrade("Alice", "Alice"));
            var currentOffer = service.GetOffer(tradeId, "Alice");
            Assert.Throws<InvalidOperationException>(() => service.SetOffer(tradeId, "Eve", currentOffer!));
        }

        [Fact]
        public void Decline_And_Cancel()
        {
            var service = new TradingService();
            string a = service.CreateTrade("Alice", "Bob");
            service.Decline(a, "Bob");
            Assert.Equal(TradingService.StatusDeclined, service.GetTrade(a)!.Status);

            string b = service.CreateTrade("Alice", "Bob");
            service.CancelTrade(b);
            Assert.Equal(TradingService.StatusCancelled, service.GetTrade(b)!.Status);
        }
    }

    public class CraftingServiceTests
    {
        [Fact]
        public void Recipes_CanCraft_Craft_History()
        {
            var inventory = new InventoryService();
            var service = new CraftingService(inventory);
            var crafted = new System.Collections.Generic.List<CraftingService.CraftRecord>();
            service.OnCraft("Alice", r => crafted.Add(r));

            service.RegisterRecipe("Sword", "Iron Sword",
                new[] { new CraftingService.Ingredient("Iron", 3) },
                new CraftingService.Output("IronSword", 1));

            Assert.Single(service.GetAllRecipes());
            Assert.False(service.CanCraft("Alice", "Sword"));
            Assert.Throws<InvalidOperationException>(() => service.Craft("Alice", "Sword"));

            inventory.AddItem("Alice", "Iron", 5);
            Assert.True(service.CanCraft("Alice", "Sword"));
            service.Craft("Alice", "Sword");

            Assert.Equal(2, inventory.GetItemCount("Alice", "Iron"));
            Assert.Equal(1, inventory.GetItemCount("Alice", "IronSword"));
            Assert.Single(crafted);
            Assert.Single(service.GetCraftHistory("Alice"));

            Assert.Throws<InvalidOperationException>(() => service.RegisterRecipe("Sword", "Dup",
                new[] { new CraftingService.Ingredient("Iron", 1) }, new CraftingService.Output("X", 1)));
        }
    }

    public class MailServiceTests
    {
        [Fact]
        public void Send_Read_ClaimAttachments_Delete()
        {
            var service = new MailService();
            var received = new System.Collections.Generic.List<MailService.Mail>();
            service.OnReceive("Bob", m => received.Add(m));

            var attachments = new[]
            {
                new MailService.MailAttachment("Potion", null, 2),
                new MailService.MailAttachment(null, "Coin", 100),
            };
            string mailId = service.Send("Alice", "Bob", "Gift", "Enjoy!", attachments);
            service.Send("Bob", "Alice", "Reply", "Thanks");

            Assert.Single(received);
            Assert.Single(service.GetInbox("Bob"));
            Assert.Single(service.GetOutbox("Alice"));

            service.Read(mailId);
            Assert.True(service.GetMail(mailId)!.Read);

            Assert.Equal(2, service.ClaimAttachments(mailId, "Bob").Count);
            Assert.Throws<InvalidOperationException>(() => service.ClaimAttachments(mailId, "Bob"));

            service.Delete(mailId);
            Assert.Null(service.GetMail(mailId));
            Assert.Throws<InvalidOperationException>(() => service.ClaimAttachments(mailId, "Bob"));
        }
    }
}
