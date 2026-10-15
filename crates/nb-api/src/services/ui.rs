/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Giao diện: UIService, InputService, TextService.
//!
//! 33 documented functions implementing presentation and interaction.

use std::collections::BTreeMap;

use nb_types::Value;
use nb_world::InstanceProperties;

use crate::args::Args;
use crate::error::{ApiError, ApiResult};
use crate::runtime::{ApiContext, InputBinding, UiElement};
use crate::services::shared::require_player;
use crate::services::unknown;

/// Routes a UI-group call.
pub fn dispatch(ctx: &ApiContext, service: &str, method: &str, args: &Args) -> ApiResult<Value> {
    match (service, method) {
        // --- InputService ---------------------------------------------
        ("InputService", "BindAction") => input_bind_action(ctx, args),
        ("InputService", "IsKeyDown") => input_is_key_down(ctx, args),
        ("InputService", "OnKeyPress") => input_on_key_press(ctx, args),
        ("InputService", "OnKeyRelease") => input_on_key_release(ctx, args),
        ("InputService", "OnMouseClick") => input_on_mouse_click(ctx, args),
        ("InputService", "OnMouseMove") => input_on_mouse_move(ctx, args),
        ("InputService", "OnTouch") => input_on_touch(ctx, args),
        ("InputService", "Remove") => input_remove(ctx, args),
        ("InputService", "UnbindAction") => input_unbind_action(ctx, args),

        // --- TextService ----------------------------------------------
        ("TextService", "CountCharacters") => text_count_characters(ctx, args),
        ("TextService", "EscapeRichText") => text_escape_rich_text(ctx, args),
        ("TextService", "Filter") => text_filter(ctx, args),
        ("TextService", "FitText") => text_fit_text(ctx, args),
        ("TextService", "MeasureText") => text_measure_text(ctx, args),
        ("TextService", "Truncate") => text_truncate(ctx, args),
        ("TextService", "WrapText") => text_wrap_text(ctx, args),

        // --- UIService ------------------------------------------------
        ("UIService", "Create") => ui_create(ctx, args),
        ("UIService", "Destroy") => ui_destroy(ctx, args),
        ("UIService", "GetCanvasSize") => ui_get_canvas_size(ctx, args),
        ("UIService", "GetProperty") => ui_get_property(ctx, args),
        ("UIService", "GetScrollPosition") => ui_get_scroll_position(ctx, args),
        ("UIService", "GetText") => ui_get_text(ctx, args),
        ("UIService", "OnClick") => ui_on_click(ctx, args),
        ("UIService", "OnTextChanged") => ui_on_text_changed(ctx, args),
        ("UIService", "RemoveCallback") => ui_remove_callback(ctx, args),
        ("UIService", "SetCanvasSize") => ui_set_canvas_size(ctx, args),
        ("UIService", "SetImage") => ui_set_image(ctx, args),
        ("UIService", "SetPlaceholder") => ui_set_placeholder(ctx, args),
        ("UIService", "SetProperty") => ui_set_property(ctx, args),
        ("UIService", "SetScrollPosition") => ui_set_scroll_position(ctx, args),
        ("UIService", "SetText") => ui_set_text(ctx, args),
        ("UIService", "SetVisible") => ui_set_visible(ctx, args),
        ("UIService", "Tween") => ui_tween(ctx, args),

        _ => Err(unknown(service, method)),
    }
}

// =========================================================================
// InputService
// =========================================================================

fn input_bind_action(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let action = args.string(0, "ActionName")?;
    let keys_val = args.at(1);
    let mut keys = Vec::new();
    if let Some(Value::Array(arr)) = keys_val {
        for v in arr {
            if let Some(s) = v.as_str() {
                keys.push(s.to_string());
            }
        }
    } else if let Some(Value::Str(s)) = keys_val {
        keys.push(s.clone());
    }
    let binding = InputBinding {
        action: action.clone(),
        keys,
        has_callback: args.at(2).is_some(),
    };
    ctx.runtime.input_bindings.write().insert(action, binding);
    Ok(Value::Bool(true))
}

fn input_is_key_down(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let key_code = args.string(1, "KeyCode")?;
    require_player(&ctx.runtime, &username)?;
    let down = ctx.runtime.keys_down.read().contains(&key_code);
    Ok(Value::Bool(down))
}

fn input_on_key_press(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let key_code = args.string(1, "KeyCode")?;
    let _ = args.at(2);
    require_player(&ctx.runtime, &username)?;
    ctx.runtime.register_callback(
        "InputService",
        "OnKeyPress",
        Some(&format!("{username}:{key_code}")),
    );
    Ok(Value::Bool(true))
}

fn input_on_key_release(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let key_code = args.string(1, "KeyCode")?;
    let _ = args.at(2);
    require_player(&ctx.runtime, &username)?;
    ctx.runtime.register_callback(
        "InputService",
        "OnKeyRelease",
        Some(&format!("{username}:{key_code}")),
    );
    Ok(Value::Bool(true))
}

fn input_on_mouse_click(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let button = args.string(1, "MouseButton")?;
    let _ = args.at(2);
    require_player(&ctx.runtime, &username)?;
    ctx.runtime.register_callback(
        "InputService",
        "OnMouseClick",
        Some(&format!("{username}:{button}")),
    );
    Ok(Value::Bool(true))
}

fn input_on_mouse_move(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let _ = args.at(1);
    require_player(&ctx.runtime, &username)?;
    ctx.runtime
        .register_callback("InputService", "OnMouseMove", Some(&username));
    Ok(Value::Bool(true))
}

fn input_on_touch(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let _ = args.at(1);
    require_player(&ctx.runtime, &username)?;
    ctx.runtime
        .register_callback("InputService", "OnTouch", Some(&username));
    Ok(Value::Bool(true))
}

fn input_remove(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let key_code = args.string(1, "KeyCode")?;
    require_player(&ctx.runtime, &username)?;
    let removed = ctx.runtime.unregister_callback(
        "InputService",
        "OnKeyPress",
        Some(&format!("{username}:{key_code}")),
    );
    let _ = ctx.runtime.unregister_callback(
        "InputService",
        "OnKeyRelease",
        Some(&format!("{username}:{key_code}")),
    );
    Ok(Value::Bool(removed))
}

fn input_unbind_action(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let action = args.string(0, "ActionName")?;
    let removed = ctx.runtime.input_bindings.write().remove(&action).is_some();
    Ok(Value::Bool(removed))
}

// =========================================================================
// TextService
// =========================================================================

fn text_count_characters(_ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let text = args.string(0, "Text")?;
    Ok(Value::from(text.chars().count() as i64))
}

fn text_escape_rich_text(_ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let text = args.string(0, "Text")?;
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;");
    Ok(Value::from(escaped))
}

fn text_filter(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let text = args.string(1, "Text")?;
    require_player(&ctx.runtime, &username)?;
    // Use the existing message filter logic
    let filtered = crate::services::player::filter_message(&text, "Normal");
    Ok(Value::from(filtered))
}

fn text_fit_text(_ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let text = args.string(0, "Text")?;
    let max_width = args.number(1, "MaxWidth")?;
    let _font = args.string(2, "Font")?;
    let font_size = args.number(3, "FontSize")?;
    // Approximate average char width is 0.6 * font_size
    let char_width = (font_size * 0.6).max(1.0);
    let max_chars = (max_width / char_width).floor() as usize;
    let chars: String = text.chars().take(max_chars).collect();
    Ok(Value::from(chars))
}

fn text_measure_text(_ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let text = args.string(0, "Text")?;
    let _font = args.string(1, "Font")?;
    let font_size = args.number(2, "FontSize")?;
    let char_width = font_size * 0.6;
    let width = text.chars().count() as f64 * char_width;
    let height = font_size * 1.2;
    Ok(Value::Map(BTreeMap::from([
        ("X".to_string(), Value::from(width)),
        ("Y".to_string(), Value::from(height)),
        ("Width".to_string(), Value::from(width)),
        ("Height".to_string(), Value::from(height)),
    ])))
}

fn text_truncate(_ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let text = args.string(0, "Text")?;
    let max_chars = args.number(1, "MaxLines")? as usize; // Treated as max length or lines
    let ellipsis = args.string_or(2, "Ellipsis", "...")?;
    if text.chars().count() > max_chars {
        let truncated: String = text.chars().take(max_chars).collect();
        Ok(Value::from(format!("{truncated}{ellipsis}")))
    } else {
        Ok(Value::from(text))
    }
}

fn text_wrap_text(_ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let text = args.string(0, "Text")?;
    let max_width = args.number(1, "MaxWidth")?;
    let _font = args.string(2, "Font")?;
    let font_size = args.number(3, "FontSize")?;
    let char_width = (font_size * 0.6).max(1.0);
    let chars_per_line = (max_width / char_width).max(1.0) as usize;

    let mut lines = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    for chunk in chars.chunks(chars_per_line) {
        lines.push(chunk.iter().collect::<String>());
    }
    Ok(Value::from(lines.join("\n")))
}

// =========================================================================
// UIService
// =========================================================================

fn ui_create(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let kind = args.string(1, "Type")?;
    let parent = args.string_or(2, "Parent", "ScreenGui")?;
    let props = args.table_or_empty(3, "Properties")?;

    let instance_props = InstanceProperties::from_value(&props);

    let elem = UiElement {
        name: name.clone(),
        kind,
        parent,
        properties: instance_props,
        has_callback: false,
    };
    ctx.runtime.ui.write().insert(name, elem);
    Ok(Value::Bool(true))
}

fn ui_destroy(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let removed = ctx.runtime.ui.write().remove(&name).is_some();
    Ok(Value::Bool(removed))
}

fn ui_get_canvas_size(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let ui = ctx.runtime.ui.read();
    let elem = ui
        .get(&name)
        .ok_or_else(|| ApiError::NotFound { kind: "ui", name })?;
    let cs = elem.properties.canvas_size;
    Ok(Value::Map(BTreeMap::from([
        ("Width".to_string(), Value::from(cs.x)),
        ("Height".to_string(), Value::from(cs.y)),
    ])))
}

fn ui_get_property(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let prop = args.string(1, "Property")?;
    let ui = ctx.runtime.ui.read();
    let elem = ui
        .get(&name)
        .ok_or_else(|| ApiError::NotFound { kind: "ui", name })?;
    Ok(elem.properties.get_named(&prop).unwrap_or(Value::Nil))
}

fn ui_get_scroll_position(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let ui = ctx.runtime.ui.read();
    let elem = ui
        .get(&name)
        .ok_or_else(|| ApiError::NotFound { kind: "ui", name })?;
    let x = elem
        .properties
        .extra
        .get("ScrollX")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let y = elem
        .properties
        .extra
        .get("ScrollY")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    Ok(Value::Map(BTreeMap::from([
        ("X".to_string(), Value::from(x)),
        ("Y".to_string(), Value::from(y)),
    ])))
}

fn ui_get_text(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let ui = ctx.runtime.ui.read();
    let elem = ui
        .get(&name)
        .ok_or_else(|| ApiError::NotFound { kind: "ui", name })?;
    Ok(Value::from(elem.properties.text.clone()))
}

fn ui_on_click(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let _ = args.at(1);
    let mut ui = ctx.runtime.ui.write();
    let elem = ui.get_mut(&name).ok_or_else(|| ApiError::NotFound {
        kind: "ui",
        name: name.clone(),
    })?;
    elem.has_callback = true;
    ctx.runtime
        .register_callback("UIService", "OnClick", Some(&name));
    Ok(Value::Bool(true))
}

fn ui_on_text_changed(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let _ = args.at(1);
    let mut ui = ctx.runtime.ui.write();
    let elem = ui.get_mut(&name).ok_or_else(|| ApiError::NotFound {
        kind: "ui",
        name: name.clone(),
    })?;
    elem.has_callback = true;
    ctx.runtime
        .register_callback("UIService", "OnTextChanged", Some(&name));
    Ok(Value::Bool(true))
}

fn ui_remove_callback(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let mut ui = ctx.runtime.ui.write();
    if let Some(elem) = ui.get_mut(&name) {
        elem.has_callback = false;
    }
    let removed = ctx
        .runtime
        .unregister_callback("UIService", "OnClick", Some(&name));
    let _ = ctx
        .runtime
        .unregister_callback("UIService", "OnTextChanged", Some(&name));
    Ok(Value::Bool(removed))
}

fn ui_set_canvas_size(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let width = args.number(1, "Width")?;
    let height = args.number(2, "Height")?;
    let mut ui = ctx.runtime.ui.write();
    let elem = ui
        .get_mut(&name)
        .ok_or_else(|| ApiError::NotFound { kind: "ui", name })?;
    elem.properties.canvas_size = nb_types::Vector3::new(width, height, 0.0);
    Ok(Value::Bool(true))
}

fn ui_set_image(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let asset_id = args.string(1, "AssetId")?;
    let mut ui = ctx.runtime.ui.write();
    let elem = ui
        .get_mut(&name)
        .ok_or_else(|| ApiError::NotFound { kind: "ui", name })?;
    elem.properties.image = asset_id;
    Ok(Value::Bool(true))
}

fn ui_set_placeholder(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let placeholder = args.string(1, "PlaceholderText")?;
    let mut ui = ctx.runtime.ui.write();
    let elem = ui
        .get_mut(&name)
        .ok_or_else(|| ApiError::NotFound { kind: "ui", name })?;
    elem.properties.placeholder = placeholder;
    Ok(Value::Bool(true))
}

fn ui_set_property(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let prop = args.string(1, "Property")?;
    let val = args.at(2).cloned().unwrap_or(Value::Nil);
    let mut ui = ctx.runtime.ui.write();
    let elem = ui
        .get_mut(&name)
        .ok_or_else(|| ApiError::NotFound { kind: "ui", name })?;
    if !elem.properties.set_named(&prop, &val) {
        elem.properties.extra.insert(prop, val);
    }
    Ok(Value::Bool(true))
}

fn ui_set_scroll_position(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let x = args.number(1, "X")?;
    let y = args.number(2, "Y")?;
    let mut ui = ctx.runtime.ui.write();
    let elem = ui
        .get_mut(&name)
        .ok_or_else(|| ApiError::NotFound { kind: "ui", name })?;
    elem.properties
        .extra
        .insert("ScrollX".to_string(), Value::from(x));
    elem.properties
        .extra
        .insert("ScrollY".to_string(), Value::from(y));
    Ok(Value::Bool(true))
}

fn ui_set_text(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let text = args.string(1, "Text")?;
    let mut ui = ctx.runtime.ui.write();
    let elem = ui
        .get_mut(&name)
        .ok_or_else(|| ApiError::NotFound { kind: "ui", name })?;
    elem.properties.text = text;
    Ok(Value::Bool(true))
}

fn ui_set_visible(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let visible = args.boolean(1, "Boolean")?;
    let mut ui = ctx.runtime.ui.write();
    let elem = ui
        .get_mut(&name)
        .ok_or_else(|| ApiError::NotFound { kind: "ui", name })?;
    elem.properties.visible = visible;
    Ok(Value::Bool(true))
}

fn ui_tween(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let props = args.table_or_empty(1, "Properties")?;
    let _time = args.number(2, "Time")?;
    let _dir = args.string_or(3, "EasingDirection", "Out")?;
    let _style = args.string_or(4, "EasingStyle", "Quad")?;

    let mut ui = ctx.runtime.ui.write();
    let elem = ui
        .get_mut(&name)
        .ok_or_else(|| ApiError::NotFound { kind: "ui", name })?;
    if let Value::Map(ref m) = props {
        for (k, v) in m {
            if !elem.properties.set_named(k, v) {
                elem.properties.extra.insert(k.clone(), v.clone());
            }
        }
    }
    Ok(Value::Bool(true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Side;
    use crate::runtime::PlayerRecord;

    fn test_ctx() -> ApiContext {
        let ctx = ApiContext::new(Side::Studio);
        ctx.runtime.player_join(PlayerRecord::new("Alice", 1, "VN"));
        ctx
    }

    #[test]
    fn text_service_helpers() {
        let ctx = test_ctx();
        let count = dispatch(
            &ctx,
            "TextService",
            "CountCharacters",
            &Args::new(
                "TextService",
                "CountCharacters",
                vec![Value::from("Hello World")],
            ),
        )
        .unwrap();
        assert_eq!(count.as_i64(), Some(11));

        let esc = dispatch(
            &ctx,
            "TextService",
            "EscapeRichText",
            &Args::new(
                "TextService",
                "EscapeRichText",
                vec![Value::from("<hello & world>")],
            ),
        )
        .unwrap();
        assert_eq!(esc.as_str(), Some("&lt;hello &amp; world&gt;"));

        let trunc = dispatch(
            &ctx,
            "TextService",
            "Truncate",
            &Args::new(
                "TextService",
                "Truncate",
                vec![
                    Value::from("SuperLongText"),
                    Value::from(5.0),
                    Value::from("..."),
                ],
            ),
        )
        .unwrap();
        assert_eq!(trunc.as_str(), Some("Super..."));
    }

    #[test]
    fn ui_lifecycle() {
        let ctx = test_ctx();
        dispatch(
            &ctx,
            "UIService",
            "Create",
            &Args::new(
                "UIService",
                "Create",
                vec![
                    Value::from("MyButton"),
                    Value::from("TextButton"),
                    Value::from("ScreenGui"),
                    Value::empty_map(),
                ],
            ),
        )
        .unwrap();

        dispatch(
            &ctx,
            "UIService",
            "SetText",
            &Args::new(
                "UIService",
                "SetText",
                vec![Value::from("MyButton"), Value::from("Click Me!")],
            ),
        )
        .unwrap();

        let text = dispatch(
            &ctx,
            "UIService",
            "GetText",
            &Args::new("UIService", "GetText", vec![Value::from("MyButton")]),
        )
        .unwrap();
        assert_eq!(text.as_str(), Some("Click Me!"));

        dispatch(
            &ctx,
            "UIService",
            "SetVisible",
            &Args::new(
                "UIService",
                "SetVisible",
                vec![Value::from("MyButton"), Value::Bool(false)],
            ),
        )
        .unwrap();

        let vis = dispatch(
            &ctx,
            "UIService",
            "GetProperty",
            &Args::new(
                "UIService",
                "GetProperty",
                vec![Value::from("MyButton"), Value::from("Visible")],
            ),
        )
        .unwrap();
        assert_eq!(vis.as_bool(), Some(false));
    }
}
