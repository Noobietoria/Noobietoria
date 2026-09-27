using System;
using System.Collections.Generic;
using System.Linq;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// CurrencyService manages player wallets for named in-game currencies
    /// ("Coin", "Gem", "EventToken", ...). Balances must never go negative.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#currencyservice
    /// </summary>
    public class CurrencyService
    {
        /// <summary>A wallet movement (OnTransaction payload).</summary>
        public sealed record Transaction(string Username, string CurrencyType, double Amount, string Kind);

        private readonly object _lock = new();
        private readonly Dictionary<string, Dictionary<string, double>> _wallets = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action<Transaction>>> _transactionCallbacks = new(StringComparer.Ordinal);

        /// <summary>CurrencyService.GetBalance(Username, CurrencyType)</summary>
        public double GetBalance(string username, string currencyType)
        {
            Require(username, currencyType);
            lock (_lock)
            {
                return Wallet(username).TryGetValue(currencyType, out var balance) ? balance : 0;
            }
        }

        /// <summary>CurrencyService.Add(Username, CurrencyType, Amount)</summary>
        public void Add(string username, string currencyType, double amount)
        {
            Require(username, currencyType);
            RequireAmount(amount);
            lock (_lock)
            {
                Wallet(username)[currencyType] = GetBalance(username, currencyType) + amount;
                Fire(username, currencyType, amount, "Add");
            }
        }

        /// <summary>CurrencyService.Deduct(Username, CurrencyType, Amount)</summary>
        public void Deduct(string username, string currencyType, double amount)
        {
            Require(username, currencyType);
            RequireAmount(amount);
            lock (_lock)
            {
                double balance = GetBalance(username, currencyType);
                if (balance < amount)
                    throw new InvalidOperationException(
                        $"Insufficient {currencyType} for '{username}': {balance} < {amount}.");
                Wallet(username)[currencyType] = balance - amount;
                Fire(username, currencyType, -amount, "Deduct");
            }
        }

        /// <summary>CurrencyService.Transfer(FromUsername, ToUsername, CurrencyType, Amount)</summary>
        public void Transfer(string fromUsername, string toUsername, string currencyType, double amount)
        {
            Require(fromUsername, currencyType);
            Require(toUsername ?? throw new ArgumentException("ToUsername must not be empty.", nameof(toUsername)), currencyType);
            RequireAmount(amount);
            lock (_lock)
            {
                double balance = GetBalance(fromUsername, currencyType);
                if (balance < amount)
                    throw new InvalidOperationException(
                        $"Insufficient {currencyType} for '{fromUsername}': {balance} < {amount}.");
                Wallet(fromUsername)[currencyType] = balance - amount;
                Wallet(toUsername)[currencyType] = GetBalance(toUsername, currencyType) + amount;
                Fire(fromUsername, currencyType, -amount, "Transfer");
                Fire(toUsername, currencyType, amount, "Transfer");
            }
        }

        /// <summary>CurrencyService.SetBalance(Username, CurrencyType, Amount)</summary>
        public void SetBalance(string username, string currencyType, double amount)
        {
            Require(username, currencyType);
            RequireAmount(amount);
            lock (_lock)
            {
                Wallet(username)[currencyType] = amount;
                Fire(username, currencyType, amount, "SetBalance");
            }
        }

        /// <summary>CurrencyService.GetAllBalances(Username)</summary>
        public IReadOnlyDictionary<string, double> GetAllBalances(string username)
        {
            Require(username, "Coin");
            lock (_lock)
            {
                return new Dictionary<string, double>(Wallet(username));
            }
        }

        /// <summary>CurrencyService.OnTransaction(Username, Callback)</summary>
        public void OnTransaction(string username, Action<Transaction> callback)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                if (!_transactionCallbacks.TryGetValue(username, out var list))
                    _transactionCallbacks[username] = list = new List<Action<Transaction>>();
                list.Add(callback);
            }
        }

        private Dictionary<string, double> Wallet(string username)
        {
            if (!_wallets.TryGetValue(username, out var wallet))
                _wallets[username] = wallet = new Dictionary<string, double>(StringComparer.Ordinal);
            return wallet;
        }

        private void Fire(string username, string currencyType, double amount, string kind)
        {
            if (!_transactionCallbacks.TryGetValue(username, out var callbacks))
                return;
            var transaction = new Transaction(username, currencyType, amount, kind);
            foreach (var callback in callbacks)
                callback(transaction);
        }

        private static void Require(string username, string currencyType)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (string.IsNullOrWhiteSpace(currencyType))
                throw new ArgumentException("CurrencyType must not be empty.", nameof(currencyType));
        }

        private static void RequireAmount(double amount)
        {
            if (amount < 0)
                throw new ArgumentException("Amount must not be negative.", nameof(amount));
        }
    }

    /// <summary>
    /// InventoryService manages players' inventories: item stacks with
    /// quantities and optional metadata.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#inventoryservice
    /// </summary>
    public class InventoryService
    {
        /// <summary>One item stack (GetInventory payload).</summary>
        public sealed record ItemStack(string ItemId, int Quantity, IReadOnlyDictionary<string, object?>? Metadata);

        private readonly object _lock = new();
        private readonly Dictionary<string, Dictionary<string, ItemStack>> _inventories = new(StringComparer.Ordinal);

        /// <summary>InventoryService.GetInventory(Username)</summary>
        public IReadOnlyList<ItemStack> GetInventory(string username)
        {
            Require(username);
            lock (_lock)
            {
                return _inventories.TryGetValue(username, out var items)
                    ? items.Values.Select(s => s with { Metadata = s.Metadata }).ToArray()
                    : Array.Empty<ItemStack>();
            }
        }

        /// <summary>InventoryService.AddItem(Username, ItemId, Quantity, Metadata)</summary>
        public void AddItem(string username, string itemId, int quantity = 1,
            IReadOnlyDictionary<string, object?>? metadata = null)
        {
            Require(username, itemId, quantity);
            lock (_lock)
            {
                var items = Items(username);
                if (items.TryGetValue(itemId, out var stack))
                    items[itemId] = stack with { Quantity = stack.Quantity + quantity };
                else
                    items[itemId] = new ItemStack(itemId, quantity, metadata);
            }
        }

        /// <summary>InventoryService.RemoveItem(Username, ItemId, Quantity)</summary>
        public void RemoveItem(string username, string itemId, int quantity = 1)
        {
            Require(username, itemId, quantity);
            lock (_lock)
            {
                var items = Items(username);
                if (!items.TryGetValue(itemId, out var stack) || stack.Quantity < quantity)
                    throw new InvalidOperationException(
                        $"'{username}' does not have {quantity} x '{itemId}'.");
                int remaining = stack.Quantity - quantity;
                if (remaining == 0)
                    items.Remove(itemId);
                else
                    items[itemId] = stack with { Quantity = remaining };
            }
        }

        /// <summary>InventoryService.HasItem(Username, ItemId)</summary>
        public bool HasItem(string username, string itemId)
        {
            Require(username, itemId);
            lock (_lock)
            {
                return Items(username).ContainsKey(itemId);
            }
        }

        /// <summary>InventoryService.GetItemCount(Username, ItemId)</summary>
        public int GetItemCount(string username, string itemId)
        {
            Require(username, itemId);
            lock (_lock)
            {
                return Items(username).TryGetValue(itemId, out var stack) ? stack.Quantity : 0;
            }
        }

        /// <summary>InventoryService.ClearInventory(Username)</summary>
        public void ClearInventory(string username)
        {
            Require(username);
            lock (_lock)
            {
                Items(username).Clear();
            }
        }

        /// <summary>InventoryService.TransferItem(FromUsername, ToUsername, ItemId, Quantity) — atomic.</summary>
        public void TransferItem(string fromUsername, string toUsername, string itemId, int quantity = 1)
        {
            Require(fromUsername, itemId, quantity);
            Require(toUsername);
            lock (_lock)
            {
                var from = Items(fromUsername);
                if (!from.TryGetValue(itemId, out var stack) || stack.Quantity < quantity)
                    throw new InvalidOperationException(
                        $"'{fromUsername}' does not have {quantity} x '{itemId}'.");

                int remaining = stack.Quantity - quantity;
                if (remaining == 0)
                    from.Remove(itemId);
                else
                    from[itemId] = stack with { Quantity = remaining };

                var to = Items(toUsername);
                if (to.TryGetValue(itemId, out var target))
                    to[itemId] = target with { Quantity = target.Quantity + quantity };
                else
                    to[itemId] = new ItemStack(itemId, quantity, stack.Metadata);
            }
        }

        private Dictionary<string, ItemStack> Items(string username)
        {
            if (!_inventories.TryGetValue(username, out var items))
                _inventories[username] = items = new Dictionary<string, ItemStack>(StringComparer.Ordinal);
            return items;
        }

        private static void Require(string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
        }

        private static void Require(string username, string itemId)
        {
            Require(username);
            if (string.IsNullOrWhiteSpace(itemId))
                throw new ArgumentException("ItemId must not be empty.", nameof(itemId));
        }

        private static void Require(string username, string itemId, int quantity)
        {
            Require(username, itemId);
            if (quantity < 1)
                throw new ArgumentException("Quantity must be at least 1.", nameof(quantity));
        }
    }

    /// <summary>
    /// MarketplaceService lists, searches and buys items on the Noobietoria
    /// marketplace.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#marketplaceservice
    /// </summary>
    public class MarketplaceService
    {
        /// <summary>One marketplace listing.</summary>
        public sealed record Listing(string ListingId, string Seller, string ItemId,
            double Price, string CurrencyType, int Quantity);

        /// <summary>One completed purchase (transaction history payload).</summary>
        public sealed record MarketTransaction(string ListingId, string Buyer, string Seller,
            string ItemId, int Quantity, double TotalPrice, string CurrencyType);

        private int _nextListingId = 1;
        private readonly object _lock = new();
        private readonly Dictionary<string, Listing> _listings = new(StringComparer.Ordinal);
        private readonly List<MarketTransaction> _transactions = new();
        private readonly Dictionary<string, List<Action<MarketTransaction>>> _saleCallbacks = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action<MarketTransaction>>> _purchaseCallbacks = new(StringComparer.Ordinal);

        /// <summary>MarketplaceService.ListItem(Seller, ItemId, Price, CurrencyType, Quantity)</summary>
        public string ListItem(string seller, string itemId, double price, string currencyType, int quantity = 1)
        {
            if (string.IsNullOrWhiteSpace(seller))
                throw new ArgumentException("Seller must not be empty.", nameof(seller));
            if (string.IsNullOrWhiteSpace(itemId))
                throw new ArgumentException("ItemId must not be empty.", nameof(itemId));
            if (price < 0)
                throw new ArgumentException("Price must not be negative.", nameof(price));
            if (string.IsNullOrWhiteSpace(currencyType))
                throw new ArgumentException("CurrencyType must not be empty.", nameof(currencyType));
            if (quantity < 1)
                throw new ArgumentException("Quantity must be at least 1.", nameof(quantity));

            lock (_lock)
            {
                string listingId = $"LST-{_nextListingId++:0000}";
                _listings[listingId] = new Listing(listingId, seller, itemId, price, currencyType, quantity);
                return listingId;
            }
        }

        /// <summary>MarketplaceService.DelistItem(ListingId)</summary>
        public void DelistItem(string listingId)
        {
            lock (_lock)
            {
                if (!_listings.Remove(listingId))
                    throw new InvalidOperationException($"No listing '{listingId}'.");
            }
        }

        /// <summary>MarketplaceService.GetListing(ListingId)</summary>
        public Listing? GetListing(string listingId)
        {
            lock (_lock)
            {
                return _listings.TryGetValue(listingId, out var listing) ? listing : null;
            }
        }

        /// <summary>MarketplaceService.GetListings(Seller)</summary>
        public IReadOnlyList<Listing> GetListings(string seller)
        {
            lock (_lock)
            {
                return _listings.Values.Where(l => l.Seller == seller).ToArray();
            }
        }

        /// <summary>
        /// MarketplaceService.Search(Query, Filters) — substring match on
        /// ItemId/Seller; supported filters: CurrencyType, MinPrice, MaxPrice.
        /// </summary>
        public IReadOnlyList<Listing> Search(string? query, IReadOnlyDictionary<string, object?>? filters = null)
        {
            lock (_lock)
            {
                IEnumerable<Listing> results = _listings.Values;
                if (!string.IsNullOrWhiteSpace(query))
                    results = results.Where(l =>
                        l.ItemId.Contains(query, StringComparison.OrdinalIgnoreCase)
                        || l.Seller.Contains(query, StringComparison.OrdinalIgnoreCase));
                if (filters != null)
                {
                    if (filters.TryGetValue("CurrencyType", out var currency) && currency is string ct)
                        results = results.Where(l => string.Equals(l.CurrencyType, ct, StringComparison.OrdinalIgnoreCase));
                    if (filters.TryGetValue("MinPrice", out var min) && min is double minPrice)
                        results = results.Where(l => l.Price >= minPrice);
                    if (filters.TryGetValue("MaxPrice", out var max) && max is double maxPrice)
                        results = results.Where(l => l.Price <= maxPrice);
                }
                return results.ToArray();
            }
        }

        /// <summary>MarketplaceService.Buy(Username, ListingId, Quantity)</summary>
        public MarketTransaction Buy(string username, string listingId, int quantity = 1)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (quantity < 1)
                throw new ArgumentException("Quantity must be at least 1.", nameof(quantity));

            MarketTransaction transaction;
            List<Action<MarketTransaction>>? saleCallbacks;
            List<Action<MarketTransaction>>? purchaseCallbacks;
            lock (_lock)
            {
                if (!_listings.TryGetValue(listingId, out var listing))
                    throw new InvalidOperationException($"No listing '{listingId}'.");
                if (listing.Quantity < quantity)
                    throw new InvalidOperationException(
                        $"Listing '{listingId}' only has {listing.Quantity} left.");
                if (listing.Seller == username)
                    throw new InvalidOperationException("You cannot buy your own listing.");

                transaction = new MarketTransaction(listingId, username, listing.Seller,
                    listing.ItemId, quantity, listing.Price * quantity, listing.CurrencyType);
                _transactions.Add(transaction);

                int remaining = listing.Quantity - quantity;
                if (remaining == 0)
                    _listings.Remove(listingId);
                else
                    _listings[listingId] = listing with { Quantity = remaining };

                _saleCallbacks.TryGetValue(listing.Seller, out saleCallbacks);
                _purchaseCallbacks.TryGetValue(username, out purchaseCallbacks);
            }
            saleCallbacks?.ForEach(cb => cb(transaction));
            purchaseCallbacks?.ForEach(cb => cb(transaction));
            return transaction;
        }

        /// <summary>MarketplaceService.GetTransactionHistory(Username, Limit)</summary>
        public IReadOnlyList<MarketTransaction> GetTransactionHistory(string username, int limit = 50)
        {
            RequireLimit(limit);
            lock (_lock)
            {
                return _transactions
                    .Where(t => t.Buyer == username || t.Seller == username)
                    .TakeLast(limit)
                    .ToArray();
            }
        }

        /// <summary>MarketplaceService.OnSale(Seller, Callback)</summary>
        public void OnSale(string seller, Action<MarketTransaction> callback) =>
            AddCallback(_saleCallbacks, seller, callback);

        /// <summary>MarketplaceService.OnPurchase(Username, Callback)</summary>
        public void OnPurchase(string username, Action<MarketTransaction> callback) =>
            AddCallback(_purchaseCallbacks, username, callback);

        private void AddCallback(Dictionary<string, List<Action<MarketTransaction>>> map, string key,
            Action<MarketTransaction> callback)
        {
            if (string.IsNullOrWhiteSpace(key))
                throw new ArgumentException("Name must not be empty.", nameof(key));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                if (!map.TryGetValue(key, out var list))
                    map[key] = list = new List<Action<MarketTransaction>>();
                list.Add(callback);
            }
        }

        internal static void RequireLimit(int limit)
        {
            if (limit < 1)
                throw new ArgumentException("Limit must be at least 1.", nameof(limit));
        }
    }

    /// <summary>
    /// PurchaseServiceIG handles transactions paid with a place-defined
    /// in-game currency (Coin, Gem, Token, ...). Orders move
    /// Pending → Confirmed/Cancelled → Refunded.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#purchaseserviceig
    /// </summary>
    public class PurchaseServiceIG
    {
        /// <summary>Order statuses.</summary>
        public static readonly string[] StatusPending = { "Pending" };
        public const string StatusConfirmed = "Confirmed";
        public const string StatusCancelled = "Cancelled";
        public const string StatusRefunded = "Refunded";

        /// <summary>One order (GetOrder / history payload).</summary>
        public sealed record Order(string OrderId, string Username, string ItemId,
            int Amount, string CurrencyType, string Status);

        private int _nextOrderId = 1;
        private readonly object _lock = new();
        private readonly Dictionary<string, Order> _orders = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action<Order>>> _purchaseCallbacks = new(StringComparer.Ordinal);

        /// <summary>PurchaseServiceIG.CreateOrder(Username, ItemId, Amount, CurrencyType)</summary>
        public string CreateOrder(string username, string itemId, int amount = 1, string currencyType = "Coin")
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (string.IsNullOrWhiteSpace(itemId))
                throw new ArgumentException("ItemId must not be empty.", nameof(itemId));
            if (amount < 1)
                throw new ArgumentException("Amount must be at least 1.", nameof(amount));
            if (string.IsNullOrWhiteSpace(currencyType))
                throw new ArgumentException("CurrencyType must not be empty.", nameof(currencyType));

            lock (_lock)
            {
                string orderId = $"IG-{_nextOrderId++:0000}";
                _orders[orderId] = new Order(orderId, username, itemId, amount, currencyType, "Pending");
                return orderId;
            }
        }

        /// <summary>PurchaseServiceIG.ConfirmOrder(OrderId)</summary>
        public void ConfirmOrder(string orderId)
        {
            Transition(orderId, "Pending", StatusConfirmed, requireConfirmed: false);
            Order? order;
            List<Action<Order>>? callbacks;
            lock (_lock)
            {
                order = _orders.GetValueOrDefault(orderId);
                _purchaseCallbacks.TryGetValue(order!.Username, out callbacks);
            }
            callbacks?.ForEach(cb => cb(order!));
        }

        /// <summary>PurchaseServiceIG.CancelOrder(OrderId)</summary>
        public void CancelOrder(string orderId) => Transition(orderId, "Pending", StatusCancelled, requireConfirmed: false);

        /// <summary>PurchaseServiceIG.Refund(OrderId) — only confirmed orders can be refunded.</summary>
        public void Refund(string orderId) => Transition(orderId, StatusConfirmed, StatusRefunded, requireConfirmed: true);

        /// <summary>PurchaseServiceIG.GetOrder(OrderId)</summary>
        public Order? GetOrder(string orderId)
        {
            lock (_lock)
            {
                return _orders.GetValueOrDefault(orderId);
            }
        }

        /// <summary>PurchaseServiceIG.GetOrderHistory(Username, Limit)</summary>
        public IReadOnlyList<Order> GetOrderHistory(string username, int limit = 50)
        {
            MarketplaceService.RequireLimit(limit);
            lock (_lock)
            {
                return _orders.Values.Where(o => o.Username == username)
                    .OrderBy(o => o.OrderId)
                    .TakeLast(limit)
                    .ToArray();
            }
        }

        /// <summary>PurchaseServiceIG.OnPurchase(Username, Callback) — fired on ConfirmOrder.</summary>
        public void OnPurchase(string username, Action<Order> callback)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                if (!_purchaseCallbacks.TryGetValue(username, out var list))
                    _purchaseCallbacks[username] = list = new List<Action<Order>>();
                list.Add(callback);
            }
        }

        private void Transition(string orderId, string from, string to, bool requireConfirmed)
        {
            if (string.IsNullOrWhiteSpace(orderId))
                throw new ArgumentException("OrderId must not be empty.", nameof(orderId));
            lock (_lock)
            {
                if (!_orders.TryGetValue(orderId, out var order))
                    throw new InvalidOperationException($"No order '{orderId}'.");
                if (!requireConfirmed && order.Status != from)
                    throw new InvalidOperationException(
                        $"Order '{orderId}' is {order.Status}, expected {from}.");
                if (requireConfirmed && order.Status != from)
                    throw new InvalidOperationException(
                        $"Order '{orderId}' is {order.Status}; only {from} orders can be refunded.");
                _orders[orderId] = order with { Status = to };
            }
        }
    }

    /// <summary>
    /// PurchaseServicePlatform handles platform-level purchases: the platform
    /// performs authentication and payment, so a Purchase completes the order
    /// (v0) and fires OnPurchase for the buyer.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#purchaseserviceplatform
    /// </summary>
    public class PurchaseServicePlatform
    {
        /// <summary>One platform order (GetOrder / history payload).</summary>
        public sealed record PlatformOrder(string OrderId, int UserId, string ItemId, string Status);

        private int _nextOrderId = 1;
        private readonly object _lock = new();
        private readonly Dictionary<string, PlatformOrder> _orders = new(StringComparer.Ordinal);
        private readonly Dictionary<int, List<Action<PlatformOrder>>> _purchaseCallbacks = new();

        /// <summary>PurchaseServicePlatform.Purchase(UserID, ItemId)</summary>
        public string Purchase(int userId, string itemId)
        {
            if (string.IsNullOrWhiteSpace(itemId))
                throw new ArgumentException("ItemId must not be empty.", nameof(itemId));

            PlatformOrder order;
            List<Action<PlatformOrder>>? callbacks;
            lock (_lock)
            {
                string orderId = $"PLT-{_nextOrderId++:0000}";
                order = new PlatformOrder(orderId, userId, itemId, "Completed");
                _orders[orderId] = order;
                _purchaseCallbacks.TryGetValue(userId, out callbacks);
            }
            callbacks?.ForEach(cb => cb(order));
            return order.OrderId;
        }

        /// <summary>PurchaseServicePlatform.GetOrder(OrderId)</summary>
        public PlatformOrder? GetOrder(string orderId)
        {
            lock (_lock)
            {
                return _orders.GetValueOrDefault(orderId);
            }
        }

        /// <summary>PurchaseServicePlatform.GetPurchaseHistory(UserID, Limit)</summary>
        public IReadOnlyList<PlatformOrder> GetPurchaseHistory(int userId, int limit = 50)
        {
            MarketplaceService.RequireLimit(limit);
            lock (_lock)
            {
                return _orders.Values.Where(o => o.UserId == userId)
                    .OrderBy(o => o.OrderId)
                    .TakeLast(limit)
                    .ToArray();
            }
        }

        /// <summary>PurchaseServicePlatform.OnPurchase(UserID, Callback)</summary>
        public void OnPurchase(int userId, Action<PlatformOrder> callback)
        {
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                if (!_purchaseCallbacks.TryGetValue(userId, out var list))
                    _purchaseCallbacks[userId] = list = new List<Action<PlatformOrder>>();
                list.Add(callback);
            }
        }
    }

    /// <summary>
    /// TradingService runs player-to-player trade sessions: both sides set an
    /// offer (items + currencies), confirm, and the trade completes when both
    /// have confirmed.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#tradingservice
    /// </summary>
    public class TradingService
    {
        /// <summary>What one side offers: item quantities + currency amounts.</summary>
        public sealed record Offer(IReadOnlyDictionary<string, int> Items, IReadOnlyDictionary<string, double> Currencies);

        /// <summary>Trade status values.</summary>
        public const string StatusPending = "Pending";
        public const string StatusCompleted = "Completed";
        public const string StatusDeclined = "Declined";
        public const string StatusCancelled = "Cancelled";

        /// <summary>One trade session (GetTrade / history payload).</summary>
        public sealed record Trade(string TradeId, string FromUsername, string ToUsername, string Status,
            Offer? FromOffer, Offer? ToOffer);

        private int _nextTradeId = 1;
        private readonly object _lock = new();
        private readonly Dictionary<string, Trade> _trades = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action<Trade>>> _completeCallbacks = new(StringComparer.Ordinal);

        /// <summary>TradingService.CreateTrade(FromUsername, ToUsername)</summary>
        public string CreateTrade(string fromUsername, string toUsername)
        {
            if (string.IsNullOrWhiteSpace(fromUsername) || string.IsNullOrWhiteSpace(toUsername))
                throw new ArgumentException("Both trade participants are required.");
            if (fromUsername == toUsername)
                throw new InvalidOperationException("A player cannot trade with themselves.");

            lock (_lock)
            {
                string tradeId = $"TRD-{_nextTradeId++:0000}";
                _trades[tradeId] = new Trade(tradeId, fromUsername, toUsername, StatusPending, null, null);
                return tradeId;
            }
        }

        /// <summary>TradingService.SetOffer(TradeId, Username, Offer)</summary>
        public void SetOffer(string tradeId, string username, Offer offer)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (offer == null)
                throw new ArgumentNullException(nameof(offer));

            lock (_lock)
            {
                var trade = RequireTrade(tradeId);
                if (trade.Status != StatusPending)
                    throw new InvalidOperationException($"Trade '{tradeId}' is {trade.Status}.");
                if (username != trade.FromUsername && username != trade.ToUsername)
                    throw new InvalidOperationException($"'{username}' is not part of trade '{tradeId}'.");

                if (username == trade.FromUsername)
                    _trades[tradeId] = trade with { FromOffer = offer };
                else
                    _trades[tradeId] = trade with { ToOffer = offer };
            }
        }

        /// <summary>TradingService.GetOffer(TradeId, Username)</summary>
        public Offer? GetOffer(string tradeId, string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            lock (_lock)
            {
                var trade = RequireTrade(tradeId);
                if (username == trade.FromUsername)
                    return trade.FromOffer;
                if (username == trade.ToUsername)
                    return trade.ToOffer;
                throw new InvalidOperationException($"'{username}' is not part of trade '{tradeId}'.");
            }
        }

        /// <summary>TradingService.Confirm(TradeId, Username) — completes the trade when both sides confirmed.</summary>
        public void Confirm(string tradeId, string username)
        {
            Trade trade;
            List<Action<Trade>>? callbacks;
            lock (_lock)
            {
                trade = RequireTrade(tradeId);
                RequireParticipant(trade, username);
                if (trade.Status != StatusPending)
                    throw new InvalidOperationException($"Trade '{tradeId}' is {trade.Status}.");

                string field = username == trade.FromUsername ? "FromConfirmed" : "ToConfirmed";
                if (!_confirmations.TryGetValue(tradeId, out var set))
                    _confirmations[tradeId] = set = new HashSet<string>(StringComparer.Ordinal);
                set.Add(field);

                if (set.Contains("FromConfirmed") && set.Contains("ToConfirmed"))
                {
                    trade = _trades[tradeId] = trade with { Status = StatusCompleted };
                }
                else
                {
                    return;
                }
                _completeCallbacks.TryGetValue(trade.FromUsername, out callbacks);
            }
            callbacks?.ForEach(cb => cb(trade));
        }

        /// <summary>TradingService.Decline(TradeId, Username)</summary>
        public void Decline(string tradeId, string username)
        {
            lock (_lock)
            {
                var trade = RequireTrade(tradeId);
                RequireParticipant(trade, username);
                if (trade.Status != StatusPending)
                    throw new InvalidOperationException($"Trade '{tradeId}' is {trade.Status}.");
                _trades[tradeId] = trade with { Status = StatusDeclined };
            }
        }

        /// <summary>TradingService.CancelTrade(TradeId)</summary>
        public void CancelTrade(string tradeId)
        {
            lock (_lock)
            {
                var trade = RequireTrade(tradeId);
                if (trade.Status != StatusPending)
                    throw new InvalidOperationException($"Trade '{tradeId}' is {trade.Status}.");
                _trades[tradeId] = trade with { Status = StatusCancelled };
            }
        }

        /// <summary>TradingService.GetTrade(TradeId)</summary>
        public Trade? GetTrade(string tradeId)
        {
            lock (_lock)
            {
                return _trades.GetValueOrDefault(tradeId);
            }
        }

        /// <summary>TradingService.GetTradeHistory(Username, Limit)</summary>
        public IReadOnlyList<Trade> GetTradeHistory(string username, int limit = 50)
        {
            MarketplaceService.RequireLimit(limit);
            lock (_lock)
            {
                return _trades.Values
                    .Where(t => t.FromUsername == username || t.ToUsername == username)
                    .OrderBy(t => t.TradeId)
                    .TakeLast(limit)
                    .ToArray();
            }
        }

        /// <summary>TradingService.OnTradeComplete(Username, Callback)</summary>
        public void OnTradeComplete(string username, Action<Trade> callback)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                if (!_completeCallbacks.TryGetValue(username, out var list))
                    _completeCallbacks[username] = list = new List<Action<Trade>>();
                list.Add(callback);
            }
        }

        private readonly Dictionary<string, HashSet<string>> _confirmations = new(StringComparer.Ordinal);

        private Trade RequireTrade(string tradeId)
        {
            if (string.IsNullOrWhiteSpace(tradeId))
                throw new ArgumentException("TradeId must not be empty.", nameof(tradeId));
            return _trades.GetValueOrDefault(tradeId)
                ?? throw new InvalidOperationException($"No trade '{tradeId}'.");
        }

        private static void RequireParticipant(Trade trade, string username)
        {
            if (username != trade.FromUsername && username != trade.ToUsername)
                throw new InvalidOperationException($"'{username}' is not part of trade '{trade.TradeId}'.");
        }
    }

    /// <summary>
    /// CraftingService registers recipes and crafts items, consuming
    /// ingredients from the player's InventoryService inventory.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#craftingservice
    /// </summary>
    public class CraftingService
    {
        /// <summary>One required ingredient.</summary>
        public sealed record Ingredient(string ItemId, int Quantity);

        /// <summary>The output a recipe produces.</summary>
        public sealed record Output(string ItemId, int Quantity, IReadOnlyDictionary<string, object?>? Metadata = null);

        /// <summary>One registered recipe (GetRecipe / GetAllRecipes).</summary>
        public sealed record Recipe(string RecipeId, string Name,
            IReadOnlyList<Ingredient> Ingredients, Output Result);

        /// <summary>One craft record (history payload).</summary>
        public sealed record CraftRecord(string RecipeId, string Username, int Quantity);

        private readonly object _lock = new();
        private readonly InventoryService _inventory;
        private readonly Dictionary<string, Recipe> _recipes = new(StringComparer.Ordinal);
        private readonly List<CraftRecord> _history = new();
        private readonly Dictionary<string, List<Action<CraftRecord>>> _craftCallbacks = new(StringComparer.Ordinal);

        public CraftingService(InventoryService? inventory = null) =>
            _inventory = inventory ?? new InventoryService();

        /// <summary>CraftingService.RegisterRecipe(RecipeId, Name, Ingredients, Output)</summary>
        public void RegisterRecipe(string recipeId, string name,
            IReadOnlyList<Ingredient> ingredients, Output output)
        {
            if (string.IsNullOrWhiteSpace(recipeId))
                throw new ArgumentException("RecipeId must not be empty.", nameof(recipeId));
            if (string.IsNullOrWhiteSpace(name))
                throw new ArgumentException("Name must not be empty.", nameof(name));
            if (ingredients == null || ingredients.Count == 0)
                throw new ArgumentException("A recipe needs at least one ingredient.", nameof(ingredients));
            if (output == null)
                throw new ArgumentNullException(nameof(output));
            if (ingredients.Any(i => i == null || string.IsNullOrWhiteSpace(i.ItemId) || i.Quantity < 1))
                throw new ArgumentException("Every ingredient needs an ItemId and a Quantity >= 1.", nameof(ingredients));
            if (output.Quantity < 1)
                throw new ArgumentException("Output quantity must be at least 1.", nameof(output));

            lock (_lock)
            {
                if (!_recipes.TryAdd(recipeId, new Recipe(recipeId, name, ingredients.ToArray(), output)))
                    throw new InvalidOperationException($"Recipe '{recipeId}' already exists.");
            }
        }

        /// <summary>CraftingService.UnregisterRecipe(RecipeId)</summary>
        public void UnregisterRecipe(string recipeId)
        {
            lock (_lock)
            {
                if (!_recipes.Remove(recipeId))
                    throw new InvalidOperationException($"No recipe '{recipeId}'.");
            }
        }

        /// <summary>CraftingService.GetRecipe(RecipeId)</summary>
        public Recipe? GetRecipe(string recipeId)
        {
            lock (_lock)
            {
                return _recipes.GetValueOrDefault(recipeId);
            }
        }

        /// <summary>CraftingService.GetAllRecipes()</summary>
        public IReadOnlyList<Recipe> GetAllRecipes()
        {
            lock (_lock)
            {
                return _recipes.Values.ToArray();
            }
        }

        /// <summary>CraftingService.CanCraft(Username, RecipeId)</summary>
        public bool CanCraft(string username, string recipeId)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            var recipe = RequireRecipe(recipeId);
            return recipe.Ingredients.All(i => _inventory.GetItemCount(username, i.ItemId) >= i.Quantity);
        }

        /// <summary>CraftingService.Craft(Username, RecipeId, Quantity) — consumes ingredients and grants the output.</summary>
        public void Craft(string username, string recipeId, int quantity = 1)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (quantity < 1)
                throw new ArgumentException("Quantity must be at least 1.", nameof(quantity));
            var recipe = RequireRecipe(recipeId);

            // Verify first so a failed craft consumes nothing.
            if (!CanCraft(username, recipeId))
                throw new InvalidOperationException(
                    $"'{username}' lacks the ingredients for recipe '{recipeId}'.");

            CraftRecord record;
            List<Action<CraftRecord>>? callbacks;
            lock (_lock)
            {
                foreach (var ingredient in recipe.Ingredients)
                    _inventory.RemoveItem(username, ingredient.ItemId, ingredient.Quantity * quantity);
                _inventory.AddItem(username, recipe.Result.ItemId, recipe.Result.Quantity * quantity,
                    recipe.Result.Metadata);

                record = new CraftRecord(recipeId, username, quantity);
                _history.Add(record);
                _craftCallbacks.TryGetValue(username, out callbacks);
            }
            callbacks?.ForEach(cb => cb(record));
        }

        /// <summary>CraftingService.GetCraftHistory(Username, Limit)</summary>
        public IReadOnlyList<CraftRecord> GetCraftHistory(string username, int limit = 50)
        {
            MarketplaceService.RequireLimit(limit);
            lock (_lock)
            {
                return _history.Where(h => h.Username == username).TakeLast(limit).ToArray();
            }
        }

        /// <summary>CraftingService.OnCraft(Username, Callback)</summary>
        public void OnCraft(string username, Action<CraftRecord> callback)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                if (!_craftCallbacks.TryGetValue(username, out var list))
                    _craftCallbacks[username] = list = new List<Action<CraftRecord>>();
                list.Add(callback);
            }
        }

        private Recipe RequireRecipe(string recipeId)
        {
            if (string.IsNullOrWhiteSpace(recipeId))
                throw new ArgumentException("RecipeId must not be empty.", nameof(recipeId));
            return _recipes.GetValueOrDefault(recipeId)
                ?? throw new InvalidOperationException($"No recipe '{recipeId}'.");
        }
    }

    /// <summary>
    /// MailService is the in-game mailbox: players send each other messages
    /// with optional item/currency attachments that can be claimed once.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#mailservice
    /// </summary>
    public class MailService
    {
        /// <summary>An attached item or currency amount.</summary>
        public sealed record MailAttachment(string? ItemId, string? CurrencyType, double Quantity);

        /// <summary>One mail (GetMail / inbox / outbox payload).</summary>
        public sealed record Mail(string MailId, string FromUsername, string ToUsername,
            string Subject, string Body, IReadOnlyList<MailAttachment> Attachments,
            bool Read, bool AttachmentsClaimed);

        private int _nextMailId = 1;
        private readonly object _lock = new();
        private readonly Dictionary<string, Mail> _mails = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action<Mail>>> _receiveCallbacks = new(StringComparer.Ordinal);

        /// <summary>MailService.Send(FromUsername, ToUsername, Subject, Body, Attachments)</summary>
        public string Send(string fromUsername, string toUsername, string subject, string body,
            IReadOnlyList<MailAttachment>? attachments = null)
        {
            if (string.IsNullOrWhiteSpace(fromUsername))
                throw new ArgumentException("FromUsername must not be empty.", nameof(fromUsername));
            if (string.IsNullOrWhiteSpace(toUsername))
                throw new ArgumentException("ToUsername must not be empty.", nameof(toUsername));
            subject ??= string.Empty;
            body ??= string.Empty;

            Mail mail;
            List<Action<Mail>>? callbacks;
            lock (_lock)
            {
                string mailId = $"MAIL-{_nextMailId++:0000}";
                mail = new Mail(mailId, fromUsername, toUsername, subject, body,
                    attachments?.ToArray() ?? Array.Empty<MailAttachment>(), false, false);
                _mails[mailId] = mail;
                _receiveCallbacks.TryGetValue(toUsername, out callbacks);
            }
            callbacks?.ForEach(cb => cb(mail));
            return mail.MailId;
        }

        /// <summary>MailService.GetInbox(Username, Limit)</summary>
        public IReadOnlyList<Mail> GetInbox(string username, int limit = 50)
        {
            MarketplaceService.RequireLimit(limit);
            lock (_lock)
            {
                return _mails.Values.Where(m => m.ToUsername == username).TakeLast(limit).ToArray();
            }
        }

        /// <summary>MailService.GetOutbox(Username, Limit)</summary>
        public IReadOnlyList<Mail> GetOutbox(string username, int limit = 50)
        {
            MarketplaceService.RequireLimit(limit);
            lock (_lock)
            {
                return _mails.Values.Where(m => m.FromUsername == username).TakeLast(limit).ToArray();
            }
        }

        /// <summary>MailService.GetMail(MailId)</summary>
        public Mail? GetMail(string mailId)
        {
            lock (_lock)
            {
                return _mails.GetValueOrDefault(mailId);
            }
        }

        /// <summary>MailService.Read(MailId) — marks the mail as read.</summary>
        public void Read(string mailId)
        {
            lock (_lock)
            {
                var mail = RequireMail(mailId);
                _mails[mailId] = mail with { Read = true };
            }
        }

        /// <summary>MailService.Delete(MailId)</summary>
        public void Delete(string mailId)
        {
            lock (_lock)
            {
                if (!_mails.Remove(mailId))
                    throw new InvalidOperationException($"No mail '{mailId}'.");
            }
        }

        /// <summary>MailService.ClaimAttachments(MailId, Username) — claims once, only as the recipient.</summary>
        public IReadOnlyList<MailAttachment> ClaimAttachments(string mailId, string username)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            lock (_lock)
            {
                var mail = RequireMail(mailId);
                if (mail.ToUsername != username)
                    throw new InvalidOperationException($"'{username}' is not the recipient of '{mailId}'.");
                if (mail.AttachmentsClaimed)
                    throw new InvalidOperationException($"Attachments of '{mailId}' were already claimed.");

                _mails[mailId] = mail with { AttachmentsClaimed = true };
                return mail.Attachments;
            }
        }

        /// <summary>MailService.OnReceive(Username, Callback)</summary>
        public void OnReceive(string username, Action<Mail> callback)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                if (!_receiveCallbacks.TryGetValue(username, out var list))
                    _receiveCallbacks[username] = list = new List<Action<Mail>>();
                list.Add(callback);
            }
        }

        private Mail RequireMail(string mailId)
        {
            if (string.IsNullOrWhiteSpace(mailId))
                throw new ArgumentException("MailId must not be empty.", nameof(mailId));
            return _mails.GetValueOrDefault(mailId)
                ?? throw new InvalidOperationException($"No mail '{mailId}'.");
        }
    }
}
