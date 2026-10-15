/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Helpers shared by more than one service group.
//!
//! Three things recur across the documentation and would otherwise be
//! re-implemented, slightly differently, in every group:
//!
//! * resolving a `Username` to a connected player, or failing precisely,
//! * reading and writing the per-player wallet map that `CurrencyService`
//!   owns and that quests, rewards, shops and mail all pay into,
//! * reading and writing the per-player inventory map, which `InventoryService`
//!   owns and that crafting, mail and rewards all touch.
//!
//! Keeping one implementation means a quest reward and a marketplace sale cannot
//! disagree about where a balance lives.

use std::collections::BTreeMap;

use nb_types::Value;

use crate::args::Args;
use crate::error::{ApiError, ApiResult};
use crate::runtime::ApiRuntime;

/// The key a player's wallet map is stored under.
pub fn wallet_key(username: &str) -> String {
    format!("{username}:Wallet")
}

/// The key a player's inventory map is stored under.
pub fn inventory_key(username: &str) -> String {
    format!("{username}:Items")
}

/// Resolves a player, or fails with the documented "no such player" error.
pub fn require_player(runtime: &ApiRuntime, username: &str) -> ApiResult<()> {
    if runtime.has_player(username) {
        Ok(())
    } else {
        Err(ApiError::NotFound {
            kind: "player",
            name: username.to_owned(),
        })
    }
}

/// Reads a player's wallet: currency name to amount.
pub fn read_wallet(runtime: &ApiRuntime, username: &str) -> ApiResult<BTreeMap<String, Value>> {
    runtime
        .wallets
        .map_or_empty(&wallet_key(username))
        .map_err(ApiError::Store)
}

/// Writes a player's wallet map back.
pub fn write_wallet(
    runtime: &ApiRuntime,
    username: &str,
    wallet: BTreeMap<String, Value>,
) -> ApiResult<()> {
    runtime
        .wallets
        .set(wallet_key(username), Value::Map(wallet))
        .map_err(ApiError::Store)
}

/// The balance for one currency, or 0 when the player has none of it.
///
/// The documentation's `GetBalance` returns a number for any currency a script
/// names, so an unknown currency is zero rather than an error.
pub fn balance(runtime: &ApiRuntime, username: &str, currency: &str) -> ApiResult<f64> {
    let wallet = read_wallet(runtime, username)?;
    Ok(match wallet.get(currency) {
        None => 0.0,
        Some(Value::Number(n)) => *n,
        Some(other) => {
            return Err(ApiError::Store(nb_store::StoreError::NotANumber {
                key: format!("{username}:{currency}"),
                actual: other.type_name(),
            }))
        }
    })
}

/// Adds `amount` to a currency and returns the new balance.
pub fn credit(runtime: &ApiRuntime, username: &str, currency: &str, amount: f64) -> ApiResult<f64> {
    let mut wallet = read_wallet(runtime, username)?;
    let current = numeric_entry(&wallet, username, currency)?;
    let next = current + amount;
    wallet.insert(currency.to_owned(), Value::from(next));
    write_wallet(runtime, username, wallet)?;
    Ok(next)
}

/// Subtracts `amount` from a currency, refusing to overdraw.
///
/// The documentation's `Deduct` is a payment, and a player who cannot afford it
/// must be told so rather than allowed to go negative — otherwise every shop in
/// the game becomes free.
pub fn debit(runtime: &ApiRuntime, username: &str, currency: &str, amount: f64) -> ApiResult<f64> {
    if amount < 0.0 {
        return Err(ApiError::Rule(format!(
            "cannot deduct a negative amount ({amount}) from {currency}"
        )));
    }
    let mut wallet = read_wallet(runtime, username)?;
    let current = numeric_entry(&wallet, username, currency)?;
    if current < amount {
        return Err(ApiError::Rule(format!(
            "{username} has {current} {currency} but {amount} was requested"
        )));
    }
    let next = current - amount;
    wallet.insert(currency.to_owned(), Value::from(next));
    write_wallet(runtime, username, wallet)?;
    Ok(next)
}

/// Sets a currency to an exact value and returns it.
pub fn set_balance(
    runtime: &ApiRuntime,
    username: &str,
    currency: &str,
    amount: f64,
) -> ApiResult<f64> {
    let mut wallet = read_wallet(runtime, username)?;
    wallet.insert(currency.to_owned(), Value::from(amount));
    write_wallet(runtime, username, wallet)?;
    Ok(amount)
}

/// Reads one currency out of a wallet map, defaulting to zero.
fn numeric_entry(
    wallet: &BTreeMap<String, Value>,
    username: &str,
    currency: &str,
) -> ApiResult<f64> {
    match wallet.get(currency) {
        None => Ok(0.0),
        Some(Value::Number(n)) => Ok(*n),
        Some(other) => Err(ApiError::Store(nb_store::StoreError::NotANumber {
            key: format!("{username}:{currency}"),
            actual: other.type_name(),
        })),
    }
}

/// Reads a player's inventory: item id to quantity.
pub fn read_inventory(runtime: &ApiRuntime, username: &str) -> ApiResult<BTreeMap<String, Value>> {
    runtime
        .inventories
        .map_or_empty(&inventory_key(username))
        .map_err(ApiError::Store)
}

/// Writes a player's inventory map back.
pub fn write_inventory(
    runtime: &ApiRuntime,
    username: &str,
    items: BTreeMap<String, Value>,
) -> ApiResult<()> {
    runtime
        .inventories
        .set(inventory_key(username), Value::Map(items))
        .map_err(ApiError::Store)
}

/// How many of an item a player holds.
pub fn item_count(runtime: &ApiRuntime, username: &str, item_id: &str) -> ApiResult<f64> {
    let items = read_inventory(runtime, username)?;
    Ok(match items.get(item_id) {
        None => 0.0,
        Some(Value::Number(n)) => *n,
        Some(other) => {
            return Err(ApiError::Store(nb_store::StoreError::NotANumber {
                key: format!("{username}:{item_id}"),
                actual: other.type_name(),
            }))
        }
    })
}

/// Adds `quantity` of an item and optionally records its metadata.
pub fn add_item(
    runtime: &ApiRuntime,
    username: &str,
    item_id: &str,
    quantity: f64,
    metadata: Option<&Value>,
) -> ApiResult<f64> {
    if quantity < 1.0 {
        return Err(ApiError::Rule(format!(
            "Quantity must be at least 1, got {quantity}"
        )));
    }
    let mut items = read_inventory(runtime, username)?;
    let current = item_count(runtime, username, item_id)?;
    let next = current + quantity;
    items.insert(item_id.to_owned(), Value::from(next));
    write_inventory(runtime, username, items)?;
    if let Some(meta) = metadata {
        if !meta.is_nil() {
            runtime
                .item_metadata
                .set(format!("{username}:{item_id}"), meta.clone())
                .map_err(ApiError::Store)?;
        }
    }
    Ok(next)
}

/// Removes `quantity` of an item, refusing to remove more than is held.
pub fn remove_item(
    runtime: &ApiRuntime,
    username: &str,
    item_id: &str,
    quantity: f64,
) -> ApiResult<f64> {
    if quantity < 1.0 {
        return Err(ApiError::Rule(format!(
            "Quantity must be at least 1, got {quantity}"
        )));
    }
    let current = item_count(runtime, username, item_id)?;
    if current < quantity {
        return Err(ApiError::Rule(format!(
            "{username} has {current} of `{item_id}` but {quantity} was requested"
        )));
    }
    let mut items = read_inventory(runtime, username)?;
    let next = current - quantity;
    if next == 0.0 {
        items.remove(item_id);
    } else {
        items.insert(item_id.to_owned(), Value::from(next));
    }
    write_inventory(runtime, username, items)?;
    Ok(next)
}

/// Validates a documented enum-ish string argument against its accepted values.
///
/// The documentation names these values exactly — `"Ascending"`, `"Descending"`,
/// `"Player"`, `"NPC"`, `"Clear"`, `"Rain"`, … — and an unrecognised value must
/// be an error, not a silent fallback.
pub fn enum_arg(args: &Args, position: usize, param: &str, accepted: &[&str]) -> ApiResult<String> {
    let raw = args.string(position, param)?;
    match accepted
        .iter()
        .find(|candidate| candidate.eq_ignore_ascii_case(&raw))
    {
        Some(matched) => Ok((*matched).to_owned()),
        None => Err(args.rule(format!(
            "`{param}` must be one of {}, got `{raw}`",
            accepted.join(", ")
        ))),
    }
}

/// Renders a receive-count as the value broadcasts return.
pub fn count(value: usize) -> Value {
    Value::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Side;
    use crate::runtime::{ApiRuntime, PlayerRecord};

    fn runtime_with_alice() -> ApiRuntime {
        let rt = ApiRuntime::new(Side::Server);
        rt.player_join(PlayerRecord::new("Alice", 1, "VN"));
        rt
    }

    #[test]
    fn require_player_rejects_an_unknown_name() {
        let rt = runtime_with_alice();
        assert!(require_player(&rt, "Alice").is_ok());
        let err = require_player(&rt, "Bob").unwrap_err();
        assert!(matches!(err, ApiError::NotFound { kind: "player", .. }));
    }

    #[test]
    fn wallet_credit_debit_and_set_round_trip() {
        let rt = runtime_with_alice();
        assert_eq!(balance(&rt, "Alice", "Coin").unwrap(), 0.0, "starts empty");

        assert_eq!(credit(&rt, "Alice", "Coin", 100.0).unwrap(), 100.0);
        assert_eq!(credit(&rt, "Alice", "Coin", 50.0).unwrap(), 150.0);
        assert_eq!(balance(&rt, "Alice", "Coin").unwrap(), 150.0);

        assert_eq!(debit(&rt, "Alice", "Coin", 25.0).unwrap(), 125.0);
        assert_eq!(set_balance(&rt, "Alice", "Coin", 7.0).unwrap(), 7.0);
        assert_eq!(balance(&rt, "Alice", "Coin").unwrap(), 7.0);
    }

    #[test]
    fn overdraft_is_refused_and_the_balance_is_untouched() {
        let rt = runtime_with_alice();
        credit(&rt, "Alice", "Coin", 30.0).unwrap();
        let err = debit(&rt, "Alice", "Coin", 100.0).unwrap_err();
        assert!(
            matches!(err, ApiError::Rule(ref m) if m.contains("30")),
            "{err}"
        );
        assert_eq!(balance(&rt, "Alice", "Coin").unwrap(), 30.0);
    }

    #[test]
    fn deducting_a_negative_amount_is_refused() {
        let rt = runtime_with_alice();
        credit(&rt, "Alice", "Coin", 10.0).unwrap();
        assert!(matches!(
            debit(&rt, "Alice", "Coin", -5.0),
            Err(ApiError::Rule(_))
        ));
        assert_eq!(balance(&rt, "Alice", "Coin").unwrap(), 10.0);
    }

    #[test]
    fn separate_currencies_do_not_bleed_into_each_other() {
        let rt = runtime_with_alice();
        credit(&rt, "Alice", "Coin", 100.0).unwrap();
        credit(&rt, "Alice", "Gem", 3.0).unwrap();
        assert_eq!(balance(&rt, "Alice", "Coin").unwrap(), 100.0);
        assert_eq!(balance(&rt, "Alice", "Gem").unwrap(), 3.0);
        assert_eq!(balance(&rt, "Alice", "Token").unwrap(), 0.0);
    }

    #[test]
    fn inventory_add_count_and_remove() {
        let rt = runtime_with_alice();
        assert_eq!(item_count(&rt, "Alice", "Sword").unwrap(), 0.0);
        assert_eq!(add_item(&rt, "Alice", "Sword", 2.0, None).unwrap(), 2.0);
        assert_eq!(item_count(&rt, "Alice", "Sword").unwrap(), 2.0);
        assert_eq!(remove_item(&rt, "Alice", "Sword", 1.0).unwrap(), 1.0);
        assert_eq!(remove_item(&rt, "Alice", "Sword", 1.0).unwrap(), 0.0);
        assert_eq!(item_count(&rt, "Alice", "Sword").unwrap(), 0.0);
        // Empty stacks are removed rather than left as zero.
        assert!(read_inventory(&rt, "Alice").unwrap().is_empty());
    }

    #[test]
    fn inventory_quantity_must_be_at_least_one() {
        let rt = runtime_with_alice();
        assert!(matches!(
            add_item(&rt, "Alice", "Sword", 0.0, None),
            Err(ApiError::Rule(_))
        ));
        assert!(matches!(
            add_item(&rt, "Alice", "Sword", -1.0, None),
            Err(ApiError::Rule(_))
        ));
        assert!(matches!(
            remove_item(&rt, "Alice", "Sword", 0.0),
            Err(ApiError::Rule(_))
        ));
    }

    #[test]
    fn removing_more_than_is_held_is_refused() {
        let rt = runtime_with_alice();
        add_item(&rt, "Alice", "Sword", 1.0, None).unwrap();
        let err = remove_item(&rt, "Alice", "Sword", 5.0).unwrap_err();
        assert!(
            matches!(err, ApiError::Rule(ref m) if m.contains("1")),
            "{err}"
        );
        assert_eq!(item_count(&rt, "Alice", "Sword").unwrap(), 1.0);
    }

    #[test]
    fn metadata_is_recorded_with_the_item() {
        let rt = runtime_with_alice();
        let meta = Value::map([("Enchant", Value::from("fire"))]);
        add_item(&rt, "Alice", "Sword", 1.0, Some(&meta)).unwrap();
        assert_eq!(
            rt.item_metadata.get("Alice:Sword"),
            Some(meta),
            "metadata is retrievable per player and item"
        );
    }

    #[test]
    fn enum_args_accept_documented_values_case_insensitively() {
        let args = Args::new(
            "LeaderboardService",
            "CreateBoard",
            vec![Value::from("Board"), Value::from("descending")],
        );
        assert_eq!(
            enum_arg(&args, 1, "Order", &["Ascending", "Descending"]).unwrap(),
            "Descending",
            "the canonical spelling is returned"
        );

        let bad = Args::new(
            "LeaderboardService",
            "CreateBoard",
            vec![Value::from("Board"), Value::from("Sideways")],
        );
        let err = enum_arg(&bad, 1, "Order", &["Ascending", "Descending"]).unwrap_err();
        assert!(
            matches!(err, ApiError::Rule(ref m) if m.contains("Sideways")),
            "{err}"
        );
    }

    #[test]
    fn inventory_is_per_player() {
        let rt = runtime_with_alice();
        rt.player_join(PlayerRecord::new("Bob", 2, "US"));
        add_item(&rt, "Alice", "Sword", 1.0, None).unwrap();
        assert_eq!(item_count(&rt, "Bob", "Sword").unwrap(), 0.0);
    }
}
