//! Vật phẩm & Kinh tế: CurrencyService, InventoryService, PurchaseService,
//! MarketplaceService, TradingService, CraftingService.
//!
//! 47 documented functions implementing the economy layer.

use std::collections::{BTreeMap, BTreeSet};

use nb_types::Value;

use crate::args::Args;
use crate::error::{ApiError, ApiResult};
use crate::runtime::{ApiContext, Listing, Order, Recipe, Trade};
use crate::services::shared::{
    add_item, balance, credit, debit, item_count, read_inventory, remove_item, require_player,
    set_balance,
};
use crate::services::unknown;

/// Routes an economy-group call.
pub fn dispatch(ctx: &ApiContext, service: &str, method: &str, args: &Args) -> ApiResult<Value> {
    match (service, method) {
        // --- CurrencyService ------------------------------------------
        ("CurrencyService", "Add") => currency_add(ctx, args),
        ("CurrencyService", "Deduct") => currency_deduct(ctx, args),
        ("CurrencyService", "GetAllBalances") => currency_get_all_balances(ctx, args),
        ("CurrencyService", "GetBalance") => currency_get_balance(ctx, args),
        ("CurrencyService", "OnTransaction") => currency_on_transaction(ctx, args),
        ("CurrencyService", "SetBalance") => currency_set_balance(ctx, args),
        ("CurrencyService", "Transfer") => currency_transfer(ctx, args),

        // --- InventoryService -----------------------------------------
        ("InventoryService", "AddItem") => inventory_add_item(ctx, args),
        ("InventoryService", "ClearInventory") => inventory_clear(ctx, args),
        ("InventoryService", "GetInventory") => inventory_get(ctx, args),
        ("InventoryService", "GetItemCount") => inventory_get_item_count(ctx, args),
        ("InventoryService", "HasItem") => inventory_has_item(ctx, args),
        ("InventoryService", "RemoveItem") => inventory_remove_item(ctx, args),
        ("InventoryService", "TransferItem") => inventory_transfer_item(ctx, args),

        // --- PurchaseService ------------------------------------------
        ("PurchaseService", "CancelOrder") => purchase_cancel_order(ctx, args),
        ("PurchaseService", "ConfirmOrder") => purchase_confirm_order(ctx, args),
        ("PurchaseService", "CreateOrder") => purchase_create_order(ctx, args),
        ("PurchaseService", "GetOrder") => purchase_get_order(ctx, args),
        ("PurchaseService", "GetOrderHistory") => purchase_get_order_history(ctx, args),
        ("PurchaseService", "OnPurchase") => purchase_on_purchase(ctx, args),
        ("PurchaseService", "Refund") => purchase_refund(ctx, args),

        // --- MarketplaceService ---------------------------------------
        ("MarketplaceService", "Buy") => market_buy(ctx, args),
        ("MarketplaceService", "DelistItem") => market_delist(ctx, args),
        ("MarketplaceService", "GetListing") => market_get_listing(ctx, args),
        ("MarketplaceService", "GetListings") => market_get_listings(ctx, args),
        ("MarketplaceService", "GetTransactionHistory") => market_get_history(ctx, args),
        ("MarketplaceService", "ListItem") => market_list_item(ctx, args),
        ("MarketplaceService", "OnPurchase") => market_on_purchase(ctx, args),
        ("MarketplaceService", "OnSale") => market_on_sale(ctx, args),
        ("MarketplaceService", "Search") => market_search(ctx, args),

        // --- TradingService -------------------------------------------
        ("TradingService", "CancelTrade") => trade_cancel(ctx, args),
        ("TradingService", "Confirm") => trade_confirm(ctx, args),
        ("TradingService", "CreateTrade") => trade_create(ctx, args),
        ("TradingService", "Decline") => trade_decline(ctx, args),
        ("TradingService", "GetOffer") => trade_get_offer(ctx, args),
        ("TradingService", "GetTrade") => trade_get_trade(ctx, args),
        ("TradingService", "GetTradeHistory") => trade_get_history(ctx, args),
        ("TradingService", "OnTradeComplete") => trade_on_complete(ctx, args),
        ("TradingService", "SetOffer") => trade_set_offer(ctx, args),

        // --- CraftingService ------------------------------------------
        ("CraftingService", "CanCraft") => craft_can_craft(ctx, args),
        ("CraftingService", "Craft") => craft_craft(ctx, args),
        ("CraftingService", "GetAllRecipes") => craft_get_all_recipes(ctx, args),
        ("CraftingService", "GetCraftHistory") => craft_get_history(ctx, args),
        ("CraftingService", "GetRecipe") => craft_get_recipe(ctx, args),
        ("CraftingService", "OnCraft") => craft_on_craft(ctx, args),
        ("CraftingService", "RegisterRecipe") => craft_register_recipe(ctx, args),
        ("CraftingService", "UnregisterRecipe") => craft_unregister_recipe(ctx, args),

        _ => Err(unknown(service, method)),
    }
}

// =========================================================================
// CurrencyService
// =========================================================================

fn currency_add(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let currency = args.string(1, "CurrencyType")?;
    let amount = args.number(2, "Amount")?;
    require_player(&ctx.runtime, &username)?;
    let new_balance = credit(&ctx.runtime, &username, &currency, amount)?;
    Ok(Value::from(new_balance))
}

fn currency_deduct(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let currency = args.string(1, "CurrencyType")?;
    let amount = args.number(2, "Amount")?;
    require_player(&ctx.runtime, &username)?;
    let new_balance = debit(&ctx.runtime, &username, &currency, amount)?;
    Ok(Value::from(new_balance))
}

fn currency_get_all_balances(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    require_player(&ctx.runtime, &username)?;
    let wallet = crate::services::shared::read_wallet(&ctx.runtime, &username)?;
    Ok(Value::Map(wallet))
}

fn currency_get_balance(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let currency = args.string(1, "CurrencyType")?;
    require_player(&ctx.runtime, &username)?;
    let bal = balance(&ctx.runtime, &username, &currency)?;
    Ok(Value::from(bal))
}

fn currency_on_transaction(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let _ = args.at(1);
    require_player(&ctx.runtime, &username)?;
    ctx.runtime
        .register_callback("CurrencyService", "OnTransaction", Some(&username));
    Ok(Value::Bool(true))
}

fn currency_set_balance(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let currency = args.string(1, "CurrencyType")?;
    let amount = args.number(2, "Amount")?;
    require_player(&ctx.runtime, &username)?;
    let bal = set_balance(&ctx.runtime, &username, &currency, amount)?;
    Ok(Value::from(bal))
}

fn currency_transfer(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let from = args.string(0, "FromUsername")?;
    let to = args.string(1, "ToUsername")?;
    let currency = args.string(2, "CurrencyType")?;
    let amount = args.number(3, "Amount")?;
    require_player(&ctx.runtime, &from)?;
    require_player(&ctx.runtime, &to)?;
    debit(&ctx.runtime, &from, &currency, amount)?;
    credit(&ctx.runtime, &to, &currency, amount)?;
    Ok(Value::Bool(true))
}

// =========================================================================
// InventoryService
// =========================================================================

fn inventory_add_item(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let item_id = args.string(1, "ItemId")?;
    let qty = args.number(2, "Quantity")?;
    let meta = args.at(3);
    require_player(&ctx.runtime, &username)?;
    let new_qty = add_item(&ctx.runtime, &username, &item_id, qty, meta)?;
    Ok(Value::from(new_qty))
}

fn inventory_clear(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    require_player(&ctx.runtime, &username)?;
    crate::services::shared::write_inventory(&ctx.runtime, &username, BTreeMap::new())?;
    Ok(Value::Bool(true))
}

fn inventory_get(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    require_player(&ctx.runtime, &username)?;
    let items = read_inventory(&ctx.runtime, &username)?;
    Ok(Value::Map(items))
}

fn inventory_get_item_count(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let item_id = args.string(1, "ItemId")?;
    require_player(&ctx.runtime, &username)?;
    let count = item_count(&ctx.runtime, &username, &item_id)?;
    Ok(Value::from(count))
}

fn inventory_has_item(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let item_id = args.string(1, "ItemId")?;
    require_player(&ctx.runtime, &username)?;
    let count = item_count(&ctx.runtime, &username, &item_id)?;
    Ok(Value::Bool(count > 0.0))
}

fn inventory_remove_item(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let item_id = args.string(1, "ItemId")?;
    let qty = args.number(2, "Quantity")?;
    require_player(&ctx.runtime, &username)?;
    let new_qty = remove_item(&ctx.runtime, &username, &item_id, qty)?;
    Ok(Value::from(new_qty))
}

fn inventory_transfer_item(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let from = args.string(0, "FromUsername")?;
    let to = args.string(1, "ToUsername")?;
    let item_id = args.string(2, "ItemId")?;
    let qty = args.number(3, "Quantity")?;
    require_player(&ctx.runtime, &from)?;
    require_player(&ctx.runtime, &to)?;
    remove_item(&ctx.runtime, &from, &item_id, qty)?;
    add_item(&ctx.runtime, &to, &item_id, qty, None)?;
    Ok(Value::Bool(true))
}

// =========================================================================
// PurchaseService
// =========================================================================

fn purchase_cancel_order(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let order_id = args.string(0, "OrderId")?;
    let mut orders = ctx.runtime.orders.write();
    let order = orders
        .get_mut(&order_id)
        .ok_or_else(|| ApiError::NotFound {
            kind: "order",
            name: order_id.clone(),
        })?;
    if order.status != "Pending" {
        return Err(ApiError::Rule(format!(
            "Order {order_id} cannot be cancelled in status {}",
            order.status
        )));
    }
    order.status = "Cancelled".to_owned();
    Ok(Value::Bool(true))
}

fn purchase_confirm_order(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let order_id = args.string(0, "OrderId")?;
    let mut orders = ctx.runtime.orders.write();
    let order = orders
        .get_mut(&order_id)
        .ok_or_else(|| ApiError::NotFound {
            kind: "order",
            name: order_id.clone(),
        })?;
    if order.status != "Pending" {
        return Err(ApiError::Rule(format!(
            "Order {order_id} cannot be confirmed in status {}",
            order.status
        )));
    }
    order.status = "Confirmed".to_owned();
    let username = order.username.clone();
    let item_id = order.item_id.clone();
    let amount = order.amount;
    drop(orders);
    add_item(&ctx.runtime, &username, &item_id, amount, None)?;
    Ok(Value::Bool(true))
}

fn purchase_create_order(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let item_id = args.string(1, "ItemId")?;
    let amount = args.number(2, "Amount")?;
    let currency = args.string(3, "CurrencyType")?;
    require_player(&ctx.runtime, &username)?;

    let id = format!(
        "ord_{}",
        ctx.runtime
            .ids
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    );
    let order = Order {
        id: id.clone(),
        username: username.clone(),
        item_id,
        amount,
        currency,
        status: "Pending".to_owned(),
        receipt: None,
    };
    ctx.runtime.orders.write().insert(id.clone(), order);

    let key = format!("{username}:Orders");
    let mut history = ctx
        .runtime
        .order_history
        .array_or_empty(&key)
        .unwrap_or_default();
    history.push(Value::from(id.clone()));
    let _ = ctx.runtime.order_history.set(key, Value::Array(history));

    Ok(Value::from(id))
}

fn purchase_get_order(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let order_id = args.string(0, "OrderId")?;
    let orders = ctx.runtime.orders.read();
    let order = orders.get(&order_id).ok_or_else(|| ApiError::NotFound {
        kind: "order",
        name: order_id,
    })?;
    Ok(Value::Map(BTreeMap::from([
        ("Id".to_string(), Value::from(order.id.clone())),
        ("Username".to_string(), Value::from(order.username.clone())),
        ("ItemId".to_string(), Value::from(order.item_id.clone())),
        ("Amount".to_string(), Value::from(order.amount)),
        ("Currency".to_string(), Value::from(order.currency.clone())),
        ("Status".to_string(), Value::from(order.status.clone())),
    ])))
}

fn purchase_get_order_history(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let limit = args.number_or(1, "Limit", 50.0)?.max(1.0) as usize;
    require_player(&ctx.runtime, &username)?;

    let key = format!("{username}:Orders");
    let ids = ctx
        .runtime
        .order_history
        .array_or_empty(&key)
        .unwrap_or_default();
    let orders = ctx.runtime.orders.read();
    let mut out = Vec::new();
    for id_val in ids.into_iter().rev().take(limit) {
        if let Some(id_str) = id_val.as_str() {
            if let Some(order) = orders.get(id_str) {
                out.push(Value::Map(BTreeMap::from([
                    ("Id".to_string(), Value::from(order.id.clone())),
                    ("ItemId".to_string(), Value::from(order.item_id.clone())),
                    ("Amount".to_string(), Value::from(order.amount)),
                    ("Currency".to_string(), Value::from(order.currency.clone())),
                    ("Status".to_string(), Value::from(order.status.clone())),
                ])));
            }
        }
    }
    Ok(Value::Array(out))
}

fn purchase_on_purchase(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let _ = args.at(1);
    require_player(&ctx.runtime, &username)?;
    ctx.runtime
        .register_callback("PurchaseService", "OnPurchase", Some(&username));
    Ok(Value::Bool(true))
}

fn purchase_refund(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let order_id = args.string(0, "OrderId")?;
    let mut orders = ctx.runtime.orders.write();
    let order = orders
        .get_mut(&order_id)
        .ok_or_else(|| ApiError::NotFound {
            kind: "order",
            name: order_id.clone(),
        })?;
    if order.status != "Confirmed" {
        return Err(ApiError::Rule(format!(
            "Order {order_id} cannot be refunded from status {}",
            order.status
        )));
    }
    order.status = "Refunded".to_owned();
    let username = order.username.clone();
    let item_id = order.item_id.clone();
    let amount = order.amount;
    drop(orders);
    let _ = remove_item(&ctx.runtime, &username, &item_id, amount);
    Ok(Value::Bool(true))
}

// =========================================================================
// MarketplaceService
// =========================================================================

fn market_buy(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let buyer = args.string(0, "Username")?;
    let listing_id = args.string(1, "ListingId")?;
    let qty = args.number(2, "Quantity")?;
    require_player(&ctx.runtime, &buyer)?;

    let mut listings = ctx.runtime.listings.write();
    let listing = listings
        .get_mut(&listing_id)
        .ok_or_else(|| ApiError::NotFound {
            kind: "listing",
            name: listing_id.clone(),
        })?;
    if !listing.active || listing.quantity < qty {
        return Err(ApiError::Rule(format!(
            "Listing {listing_id} has insufficient stock (available: {})",
            listing.quantity
        )));
    }

    let total_cost = listing.price * qty;
    debit(&ctx.runtime, &buyer, &listing.currency, total_cost)?;
    credit(&ctx.runtime, &listing.seller, &listing.currency, total_cost)?;
    add_item(&ctx.runtime, &buyer, &listing.item_id, qty, None)?;

    listing.quantity -= qty;
    if listing.quantity <= 0.0 {
        listing.active = false;
    }
    Ok(Value::Bool(true))
}

fn market_delist(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let listing_id = args.string(0, "ListingId")?;
    let mut listings = ctx.runtime.listings.write();
    let listing = listings
        .get_mut(&listing_id)
        .ok_or_else(|| ApiError::NotFound {
            kind: "listing",
            name: listing_id.clone(),
        })?;
    listing.active = false;
    if listing.quantity > 0.0 {
        let _ = add_item(
            &ctx.runtime,
            &listing.seller,
            &listing.item_id,
            listing.quantity,
            None,
        );
        listing.quantity = 0.0;
    }
    Ok(Value::Bool(true))
}

fn market_get_listing(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let listing_id = args.string(0, "ListingId")?;
    let listings = ctx.runtime.listings.read();
    let l = listings
        .get(&listing_id)
        .ok_or_else(|| ApiError::NotFound {
            kind: "listing",
            name: listing_id,
        })?;
    Ok(Value::Map(BTreeMap::from([
        ("Id".to_string(), Value::from(l.id.clone())),
        ("Seller".to_string(), Value::from(l.seller.clone())),
        ("ItemId".to_string(), Value::from(l.item_id.clone())),
        ("Price".to_string(), Value::from(l.price)),
        ("Currency".to_string(), Value::from(l.currency.clone())),
        ("Quantity".to_string(), Value::from(l.quantity)),
        ("Active".to_string(), Value::Bool(l.active)),
    ])))
}

fn market_get_listings(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let seller = args.string(0, "Seller")?;
    let listings = ctx.runtime.listings.read();
    let mut out = Vec::new();
    for l in listings.values().filter(|l| l.seller == seller) {
        out.push(Value::Map(BTreeMap::from([
            ("Id".to_string(), Value::from(l.id.clone())),
            ("ItemId".to_string(), Value::from(l.item_id.clone())),
            ("Price".to_string(), Value::from(l.price)),
            ("Currency".to_string(), Value::from(l.currency.clone())),
            ("Quantity".to_string(), Value::from(l.quantity)),
            ("Active".to_string(), Value::Bool(l.active)),
        ])));
    }
    Ok(Value::Array(out))
}

fn market_get_history(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let _limit = args.number_or(1, "Limit", 50.0)? as usize;
    require_player(&ctx.runtime, &username)?;
    let key = format!("{username}:MarketHistory");
    let history = ctx
        .runtime
        .purchases
        .array_or_empty(&key)
        .unwrap_or_default();
    Ok(Value::Array(history))
}

fn market_list_item(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let seller = args.string(0, "Seller")?;
    let item_id = args.string(1, "ItemId")?;
    let price = args.number(2, "Price")?;
    let currency = args.string(3, "CurrencyType")?;
    let qty = args.number(4, "Quantity")?;
    require_player(&ctx.runtime, &seller)?;

    remove_item(&ctx.runtime, &seller, &item_id, qty)?;

    let id = format!(
        "list_{}",
        ctx.runtime
            .ids
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    );
    let listing = Listing {
        id: id.clone(),
        seller,
        item_id,
        price,
        currency,
        quantity: qty,
        active: true,
    };
    ctx.runtime.listings.write().insert(id.clone(), listing);
    Ok(Value::from(id))
}

fn market_on_purchase(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let _ = args.at(1);
    require_player(&ctx.runtime, &username)?;
    ctx.runtime
        .register_callback("MarketplaceService", "OnPurchase", Some(&username));
    Ok(Value::Bool(true))
}

fn market_on_sale(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let seller = args.string(0, "Seller")?;
    let _ = args.at(1);
    require_player(&ctx.runtime, &seller)?;
    ctx.runtime
        .register_callback("MarketplaceService", "OnSale", Some(&seller));
    Ok(Value::Bool(true))
}

fn market_search(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let query = args.string(0, "Query")?;
    let _filters = args.at(1);
    let listings = ctx.runtime.listings.read();
    let query_lower = query.to_lowercase();
    let mut out = Vec::new();
    for l in listings.values().filter(|l| {
        l.active && (query.is_empty() || l.item_id.to_lowercase().contains(&query_lower))
    }) {
        out.push(Value::Map(BTreeMap::from([
            ("Id".to_string(), Value::from(l.id.clone())),
            ("Seller".to_string(), Value::from(l.seller.clone())),
            ("ItemId".to_string(), Value::from(l.item_id.clone())),
            ("Price".to_string(), Value::from(l.price)),
            ("Currency".to_string(), Value::from(l.currency.clone())),
            ("Quantity".to_string(), Value::from(l.quantity)),
        ])));
    }
    Ok(Value::Array(out))
}

// =========================================================================
// TradingService
// =========================================================================

fn trade_cancel(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let trade_id = args.string(0, "TradeId")?;
    let mut trades = ctx.runtime.trades.write();
    let trade = trades
        .get_mut(&trade_id)
        .ok_or_else(|| ApiError::NotFound {
            kind: "trade",
            name: trade_id.clone(),
        })?;
    trade.status = "Cancelled".to_owned();
    Ok(Value::Bool(true))
}

fn trade_confirm(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let trade_id = args.string(0, "TradeId")?;
    let username = args.string(1, "Username")?;
    require_player(&ctx.runtime, &username)?;

    let mut trades = ctx.runtime.trades.write();
    let trade = trades
        .get_mut(&trade_id)
        .ok_or_else(|| ApiError::NotFound {
            kind: "trade",
            name: trade_id.clone(),
        })?;
    if trade.status != "Open" {
        return Err(ApiError::Rule(format!("Trade {trade_id} is not open")));
    }
    trade.confirmed.insert(username);

    if trade.confirmed.contains(&trade.parties.0) && trade.confirmed.contains(&trade.parties.1) {
        trade.status = "Completed".to_owned();
        let p0 = trade.parties.0.clone();
        let p1 = trade.parties.1.clone();
        let _ = ctx
            .runtime
            .trade_history
            .set(format!("{p0}:Trades:{trade_id}"), Value::from("Completed"));
        let _ = ctx
            .runtime
            .trade_history
            .set(format!("{p1}:Trades:{trade_id}"), Value::from("Completed"));
    }
    Ok(Value::Bool(true))
}

fn trade_create(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let from = args.string(0, "FromUsername")?;
    let to = args.string(1, "ToUsername")?;
    require_player(&ctx.runtime, &from)?;
    require_player(&ctx.runtime, &to)?;

    let id = format!(
        "tr_{}",
        ctx.runtime
            .ids
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    );
    let trade = Trade {
        id: id.clone(),
        parties: (from.clone(), to.clone()),
        offers: BTreeMap::new(),
        confirmed: BTreeSet::new(),
        status: "Open".to_owned(),
    };
    ctx.runtime.trades.write().insert(id.clone(), trade);
    Ok(Value::from(id))
}

fn trade_decline(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let trade_id = args.string(0, "TradeId")?;
    let _username = args.string(1, "Username")?;
    let mut trades = ctx.runtime.trades.write();
    let trade = trades
        .get_mut(&trade_id)
        .ok_or_else(|| ApiError::NotFound {
            kind: "trade",
            name: trade_id.clone(),
        })?;
    trade.status = "Declined".to_owned();
    Ok(Value::Bool(true))
}

fn trade_get_offer(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let trade_id = args.string(0, "TradeId")?;
    let username = args.string(1, "Username")?;
    let trades = ctx.runtime.trades.read();
    let trade = trades.get(&trade_id).ok_or_else(|| ApiError::NotFound {
        kind: "trade",
        name: trade_id,
    })?;
    Ok(trade.offers.get(&username).cloned().unwrap_or(Value::Nil))
}

fn trade_get_trade(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let trade_id = args.string(0, "TradeId")?;
    let trades = ctx.runtime.trades.read();
    let trade = trades.get(&trade_id).ok_or_else(|| ApiError::NotFound {
        kind: "trade",
        name: trade_id,
    })?;
    Ok(Value::Map(BTreeMap::from([
        ("Id".to_string(), Value::from(trade.id.clone())),
        ("From".to_string(), Value::from(trade.parties.0.clone())),
        ("To".to_string(), Value::from(trade.parties.1.clone())),
        ("Status".to_string(), Value::from(trade.status.clone())),
    ])))
}

fn trade_get_history(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let _limit = args.number_or(1, "Limit", 50.0)?;
    require_player(&ctx.runtime, &username)?;
    let trades = ctx.runtime.trades.read();
    let mut out = Vec::new();
    for t in trades
        .values()
        .filter(|t| t.parties.0 == username || t.parties.1 == username)
    {
        out.push(Value::Map(BTreeMap::from([
            ("Id".to_string(), Value::from(t.id.clone())),
            ("Status".to_string(), Value::from(t.status.clone())),
        ])));
    }
    Ok(Value::Array(out))
}

fn trade_on_complete(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let _ = args.at(1);
    require_player(&ctx.runtime, &username)?;
    ctx.runtime
        .register_callback("TradingService", "OnTradeComplete", Some(&username));
    Ok(Value::Bool(true))
}

fn trade_set_offer(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let trade_id = args.string(0, "TradeId")?;
    let username = args.string(1, "Username")?;
    let offer = args.at(2).cloned().unwrap_or(Value::Nil);
    require_player(&ctx.runtime, &username)?;

    let mut trades = ctx.runtime.trades.write();
    let trade = trades
        .get_mut(&trade_id)
        .ok_or_else(|| ApiError::NotFound {
            kind: "trade",
            name: trade_id.clone(),
        })?;
    trade.offers.insert(username, offer);
    trade.confirmed.clear();
    Ok(Value::Bool(true))
}

// =========================================================================
// CraftingService
// =========================================================================

fn craft_can_craft(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let recipe_id = args.string(1, "RecipeId")?;
    require_player(&ctx.runtime, &username)?;

    let recipes = ctx.runtime.recipes.read();
    let recipe = match recipes.get(&recipe_id) {
        Some(r) => r,
        None => return Ok(Value::Bool(false)),
    };

    if let Value::Map(ref ingredients) = recipe.ingredients {
        for (item_id, req_qty_val) in ingredients {
            let req_qty = req_qty_val.as_f64().unwrap_or(1.0);
            let has = item_count(&ctx.runtime, &username, item_id)?;
            if has < req_qty {
                return Ok(Value::Bool(false));
            }
        }
    }
    Ok(Value::Bool(true))
}

fn craft_craft(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let recipe_id = args.string(1, "RecipeId")?;
    let qty = args.number_or(2, "Quantity", 1.0)?.max(1.0);
    require_player(&ctx.runtime, &username)?;

    let recipes = ctx.runtime.recipes.read();
    let recipe = recipes.get(&recipe_id).ok_or_else(|| ApiError::NotFound {
        kind: "recipe",
        name: recipe_id.clone(),
    })?;

    if let Value::Map(ref ingredients) = recipe.ingredients {
        for (item_id, req_qty_val) in ingredients {
            let req_qty = req_qty_val.as_f64().unwrap_or(1.0) * qty;
            let has = item_count(&ctx.runtime, &username, item_id)?;
            if has < req_qty {
                return Err(ApiError::Rule(format!(
                    "Missing ingredient {item_id}: have {has}, need {req_qty}"
                )));
            }
        }
        for (item_id, req_qty_val) in ingredients {
            let req_qty = req_qty_val.as_f64().unwrap_or(1.0) * qty;
            remove_item(&ctx.runtime, &username, item_id, req_qty)?;
        }
    }

    let output = recipe.output.clone();
    drop(recipes);
    if let Value::Str(ref out_item) = output {
        add_item(&ctx.runtime, &username, out_item, qty, None)?;
    } else if let Value::Map(ref out_map) = output {
        for (out_item, out_qty) in out_map {
            add_item(
                &ctx.runtime,
                &username,
                out_item,
                out_qty.as_f64().unwrap_or(1.0) * qty,
                None,
            )?;
        }
    }

    let key = format!("{username}:CraftHistory");
    let mut hist = ctx
        .runtime
        .craft_history
        .array_or_empty(&key)
        .unwrap_or_default();
    hist.push(Value::from(recipe_id));
    let _ = ctx.runtime.craft_history.set(key, Value::Array(hist));

    Ok(Value::Bool(true))
}

fn craft_get_all_recipes(ctx: &ApiContext, _args: &Args) -> ApiResult<Value> {
    let recipes = ctx.runtime.recipes.read();
    let mut out = Vec::new();
    for r in recipes.values() {
        out.push(Value::Map(BTreeMap::from([
            ("Id".to_string(), Value::from(r.id.clone())),
            ("Name".to_string(), Value::from(r.name.clone())),
            ("Ingredients".to_string(), r.ingredients.clone()),
            ("Output".to_string(), r.output.clone()),
        ])));
    }
    Ok(Value::Array(out))
}

fn craft_get_history(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let _limit = args.optional_number(1, "Limit");
    require_player(&ctx.runtime, &username)?;
    let key = format!("{username}:CraftHistory");
    let hist = ctx
        .runtime
        .craft_history
        .array_or_empty(&key)
        .unwrap_or_default();
    Ok(Value::Array(hist))
}

fn craft_get_recipe(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let recipe_id = args.string(0, "RecipeId")?;
    let recipes = ctx.runtime.recipes.read();
    let r = recipes.get(&recipe_id).ok_or_else(|| ApiError::NotFound {
        kind: "recipe",
        name: recipe_id,
    })?;
    Ok(Value::Map(BTreeMap::from([
        ("Id".to_string(), Value::from(r.id.clone())),
        ("Name".to_string(), Value::from(r.name.clone())),
        ("Ingredients".to_string(), r.ingredients.clone()),
        ("Output".to_string(), r.output.clone()),
    ])))
}

fn craft_on_craft(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let _ = args.at(1);
    require_player(&ctx.runtime, &username)?;
    ctx.runtime
        .register_callback("CraftingService", "OnCraft", Some(&username));
    Ok(Value::Bool(true))
}

fn craft_register_recipe(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let recipe_id = args.string(0, "RecipeId")?;
    let name = args.string(1, "Name")?;
    let ingredients = args.table_or_empty(2, "Ingredients")?;
    let output = args.at(3).cloned().unwrap_or(Value::Nil);

    let recipe = Recipe {
        id: recipe_id.clone(),
        name,
        ingredients,
        output,
    };
    ctx.runtime.recipes.write().insert(recipe_id, recipe);
    Ok(Value::Bool(true))
}

fn craft_unregister_recipe(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let recipe_id = args.string(0, "RecipeId")?;
    let removed = ctx.runtime.recipes.write().remove(&recipe_id).is_some();
    Ok(Value::Bool(removed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Side;
    use crate::runtime::PlayerRecord;

    fn test_ctx() -> ApiContext {
        let ctx = ApiContext::new(Side::Server);
        ctx.runtime.player_join(PlayerRecord::new("Alice", 1, "VN"));
        ctx.runtime.player_join(PlayerRecord::new("Bob", 2, "US"));
        ctx
    }

    #[test]
    fn currency_lifecycle() {
        let ctx = test_ctx();
        let add = dispatch(
            &ctx,
            "CurrencyService",
            "Add",
            &Args::new(
                "CurrencyService",
                "Add",
                vec![
                    Value::from("Alice"),
                    Value::from("Gold"),
                    Value::from(100.0),
                ],
            ),
        )
        .unwrap();
        assert_eq!(add.as_f64(), Some(100.0));

        let bal = dispatch(
            &ctx,
            "CurrencyService",
            "GetBalance",
            &Args::new(
                "CurrencyService",
                "GetBalance",
                vec![Value::from("Alice"), Value::from("Gold")],
            ),
        )
        .unwrap();
        assert_eq!(bal.as_f64(), Some(100.0));

        dispatch(
            &ctx,
            "CurrencyService",
            "Transfer",
            &Args::new(
                "CurrencyService",
                "Transfer",
                vec![
                    Value::from("Alice"),
                    Value::from("Bob"),
                    Value::from("Gold"),
                    Value::from(40.0),
                ],
            ),
        )
        .unwrap();

        let bal_alice = dispatch(
            &ctx,
            "CurrencyService",
            "GetBalance",
            &Args::new(
                "CurrencyService",
                "GetBalance",
                vec![Value::from("Alice"), Value::from("Gold")],
            ),
        )
        .unwrap();
        let bal_bob = dispatch(
            &ctx,
            "CurrencyService",
            "GetBalance",
            &Args::new(
                "CurrencyService",
                "GetBalance",
                vec![Value::from("Bob"), Value::from("Gold")],
            ),
        )
        .unwrap();
        assert_eq!(bal_alice.as_f64(), Some(60.0));
        assert_eq!(bal_bob.as_f64(), Some(40.0));
    }

    #[test]
    fn inventory_lifecycle() {
        let ctx = test_ctx();
        dispatch(
            &ctx,
            "InventoryService",
            "AddItem",
            &Args::new(
                "InventoryService",
                "AddItem",
                vec![
                    Value::from("Alice"),
                    Value::from("Potion"),
                    Value::from(5.0),
                ],
            ),
        )
        .unwrap();

        let has = dispatch(
            &ctx,
            "InventoryService",
            "HasItem",
            &Args::new(
                "InventoryService",
                "HasItem",
                vec![Value::from("Alice"), Value::from("Potion")],
            ),
        )
        .unwrap();
        assert_eq!(has.as_bool(), Some(true));

        let count = dispatch(
            &ctx,
            "InventoryService",
            "GetItemCount",
            &Args::new(
                "InventoryService",
                "GetItemCount",
                vec![Value::from("Alice"), Value::from("Potion")],
            ),
        )
        .unwrap();
        assert_eq!(count.as_f64(), Some(5.0));

        dispatch(
            &ctx,
            "InventoryService",
            "TransferItem",
            &Args::new(
                "InventoryService",
                "TransferItem",
                vec![
                    Value::from("Alice"),
                    Value::from("Bob"),
                    Value::from("Potion"),
                    Value::from(2.0),
                ],
            ),
        )
        .unwrap();

        let count_alice = dispatch(
            &ctx,
            "InventoryService",
            "GetItemCount",
            &Args::new(
                "InventoryService",
                "GetItemCount",
                vec![Value::from("Alice"), Value::from("Potion")],
            ),
        )
        .unwrap();
        let count_bob = dispatch(
            &ctx,
            "InventoryService",
            "GetItemCount",
            &Args::new(
                "InventoryService",
                "GetItemCount",
                vec![Value::from("Bob"), Value::from("Potion")],
            ),
        )
        .unwrap();
        assert_eq!(count_alice.as_f64(), Some(3.0));
        assert_eq!(count_bob.as_f64(), Some(2.0));
    }

    #[test]
    fn crafting_lifecycle() {
        let ctx = test_ctx();
        let ingredients = Value::map([("Wood", Value::from(2.0)), ("Iron", Value::from(1.0))]);
        let output = Value::from("Sword");

        dispatch(
            &ctx,
            "CraftingService",
            "RegisterRecipe",
            &Args::new(
                "CraftingService",
                "RegisterRecipe",
                vec![
                    Value::from("craft_sword"),
                    Value::from("Iron Sword"),
                    ingredients,
                    output,
                ],
            ),
        )
        .unwrap();

        dispatch(
            &ctx,
            "InventoryService",
            "AddItem",
            &Args::new(
                "InventoryService",
                "AddItem",
                vec![Value::from("Alice"), Value::from("Wood"), Value::from(5.0)],
            ),
        )
        .unwrap();
        dispatch(
            &ctx,
            "InventoryService",
            "AddItem",
            &Args::new(
                "InventoryService",
                "AddItem",
                vec![Value::from("Alice"), Value::from("Iron"), Value::from(3.0)],
            ),
        )
        .unwrap();

        let can = dispatch(
            &ctx,
            "CraftingService",
            "CanCraft",
            &Args::new(
                "CraftingService",
                "CanCraft",
                vec![Value::from("Alice"), Value::from("craft_sword")],
            ),
        )
        .unwrap();
        assert_eq!(can.as_bool(), Some(true));

        dispatch(
            &ctx,
            "CraftingService",
            "Craft",
            &Args::new(
                "CraftingService",
                "Craft",
                vec![
                    Value::from("Alice"),
                    Value::from("craft_sword"),
                    Value::from(1.0),
                ],
            ),
        )
        .unwrap();

        let swords = dispatch(
            &ctx,
            "InventoryService",
            "GetItemCount",
            &Args::new(
                "InventoryService",
                "GetItemCount",
                vec![Value::from("Alice"), Value::from("Sword")],
            ),
        )
        .unwrap();
        assert_eq!(swords.as_f64(), Some(1.0));
    }
}
