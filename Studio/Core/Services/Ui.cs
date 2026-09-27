using System;
using System.Collections.Generic;
using System.Linq;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// UIService creates and manages basic UI elements (Frame, TextLabel,
    /// TextButton, ImageLabel, ImageButton, TextBox, ScrollingFrame) for the
    /// client. Element names are unique; a root element's Parent is
    /// "ScreenGui". Tweening is recorded for the engine to interpolate.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#uiservice
    /// </summary>
    public class UIService
    {
        /// <summary>UI element types supported, matching the docs.</summary>
        public static readonly IReadOnlyList<string> SupportedTypes =
            new[] { "Frame", "TextLabel", "TextButton", "ImageLabel", "ImageButton", "TextBox", "ScrollingFrame" };

        /// <summary>One UI element.</summary>
        public sealed record UiElement(string Name, string Type, string Parent, bool Visible,
            IReadOnlyDictionary<string, object?> Properties);

        /// <summary>A recorded tween (SetCanvasSize/SetScrollPosition/Get* are live state).</summary>
        public sealed record UiTween(string Name, IReadOnlyDictionary<string, object?> Properties,
            double Time, string? EasingDirection, string? EasingStyle);

        private static readonly HashSet<string> TextTypes =
            new(StringComparer.OrdinalIgnoreCase) { "TextLabel", "TextButton", "TextBox" };
        private static readonly HashSet<string> ImageTypes =
            new(StringComparer.OrdinalIgnoreCase) { "ImageLabel", "ImageButton" };

        private readonly object _lock = new();
        private readonly Dictionary<string, UiElement> _elements = new(StringComparer.Ordinal);
        private readonly Dictionary<string, Dictionary<string, object?>> _live = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action>> _clickCallbacks = new(StringComparer.Ordinal);
        private readonly Dictionary<string, List<Action<string?>>> _textChangedCallbacks = new(StringComparer.Ordinal);
        private readonly List<UiTween> _tweens = new();

        /// <summary>UIService.Create(Name, Type, Parent, Properties)</summary>
        public void Create(string name, string type, string parent = "ScreenGui",
            IReadOnlyDictionary<string, object?>? properties = null)
        {
            RequireName(name);
            if (!SupportedTypes.Any(t => string.Equals(t, type, StringComparison.OrdinalIgnoreCase)))
                throw new ArgumentException(
                    $"UIService only supports {string.Join(", ", SupportedTypes)}.", nameof(type));

            lock (_lock)
            {
                if (!_elements.TryAdd(name,
                        new UiElement(name, NormalizeType(type), parent, true, Copy(properties))))
                    throw new InvalidOperationException($"UI element '{name}' already exists.");
                if (properties != null)
                    _live[name] = new Dictionary<string, object?>(properties);
            }
        }

        /// <summary>UIService.Destroy(Name)</summary>
        public void Destroy(string name)
        {
            RequireName(name);
            lock (_lock)
            {
                if (!_elements.Remove(name))
                    throw new InvalidOperationException($"No UI element '{name}'.");
                _live.Remove(name);
                _clickCallbacks.Remove(name);
                _textChangedCallbacks.Remove(name);
            }
        }

        /// <summary>UIService.SetProperty(Name, Property, Value)</summary>
        public void SetProperty(string name, string property, object? value)
        {
            RequireName(name);
            if (string.IsNullOrWhiteSpace(property))
                throw new ArgumentException("Property must not be empty.", nameof(property));
            lock (_lock)
            {
                RequireElement(name);
                Live(name)[property] = value;
                _elements[name] = _elements[name] with { Properties = Copy(Live(name)) };
            }
        }

        /// <summary>UIService.GetProperty(Name, Property)</summary>
        public object? GetProperty(string name, string property)
        {
            RequireName(name);
            lock (_lock)
            {
                RequireElement(name);
                return Live(name).TryGetValue(property, out var value) ? value : null;
            }
        }

        /// <summary>UIService.SetVisible(Name, Boolean)</summary>
        public void SetVisible(string name, bool visible)
        {
            RequireName(name);
            lock (_lock)
            {
                RequireElement(name);
                _elements[name] = _elements[name] with { Visible = visible };
            }
        }

        /// <summary>
        /// UIService.Tween(Name, Properties, Time, EasingDirection,
        /// EasingStyle) — records a property tween for the engine.
        /// </summary>
        public void Tween(string name, IReadOnlyDictionary<string, object?> properties, double time,
            string? easingDirection = null, string? easingStyle = null)
        {
            RequireName(name);
            if (properties == null || properties.Count == 0)
                throw new ArgumentException("Tween properties must not be empty.", nameof(properties));
            if (time <= 0)
                throw new ArgumentException("Time must be greater than 0.", nameof(time));

            lock (_lock)
            {
                RequireElement(name);
                _tweens.Add(new UiTween(name, Copy(properties), time, easingDirection, easingStyle));
            }
        }

        /// <summary>UIService.OnClick(Name, Callback)</summary>
        public void OnClick(string name, Action callback)
        {
            RequireName(name);
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                RequireElement(name);
                AddCallback(_clickCallbacks, name, callback);
            }
        }

        /// <summary>UIService.OnTextChanged(Name, Callback) — receives the new text.</summary>
        public void OnTextChanged(string name, Action<string?> callback)
        {
            RequireName(name);
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                RequireElement(name);
                if (!_textChangedCallbacks.TryGetValue(name, out var list))
                    _textChangedCallbacks[name] = list = new List<Action<string?>>();
                list.Add(callback);
            }
        }

        /// <summary>UIService.RemoveCallback(Name) — removes the element's registered callbacks.</summary>
        public void RemoveCallback(string name)
        {
            RequireName(name);
            lock (_lock)
            {
                RequireElement(name);
                _clickCallbacks.Remove(name);
                _textChangedCallbacks.Remove(name);
            }
        }

        /// <summary>UIService.SetText(Name, Text) — text elements only.</summary>
        public void SetText(string name, string? text)
        {
            RequireKind(name, TextTypes, "SetText");
            lock (_lock)
            {
                Live(name)["Text"] = text;
            }
        }

        /// <summary>UIService.GetText(Name)</summary>
        public string? GetText(string name)
        {
            RequireKind(name, TextTypes, "GetText");
            lock (_lock)
            {
                return Live(name).TryGetValue("Text", out var value) ? value as string : null;
            }
        }

        /// <summary>UIService.SetImage(Name, AssetId) — image elements only; AssetId from NImageAssets.</summary>
        public void SetImage(string name, string? assetId)
        {
            RequireKind(name, ImageTypes, "SetImage");
            lock (_lock)
            {
                Live(name)["Image"] = assetId;
            }
        }

        /// <summary>UIService.SetPlaceholder(Name, PlaceholderText) — TextBox only.</summary>
        public void SetPlaceholder(string name, string? placeholder)
        {
            RequireKind(name, new HashSet<string> { "TextBox" }, "SetPlaceholder");
            lock (_lock)
            {
                Live(name)["PlaceholderText"] = placeholder;
            }
        }

        /// <summary>UIService.SetCanvasSize(Name, Width, Height) — ScrollingFrame only.</summary>
        public void SetCanvasSize(string name, double width, double height)
        {
            RequireScrollingFrame(name);
            lock (_lock)
            {
                Live(name)["CanvasSize"] = new Vector3Data(width, height, 0);
            }
        }

        /// <summary>UIService.GetCanvasSize(Name)</summary>
        public Vector3Data? GetCanvasSize(string name)
        {
            RequireScrollingFrame(name);
            lock (_lock)
            {
                return Live(name).TryGetValue("CanvasSize", out var value) && value is Vector3Data size
                    ? size
                    : null;
            }
        }

        /// <summary>UIService.SetScrollPosition(Name, X, Y) — ScrollingFrame only.</summary>
        public void SetScrollPosition(string name, double x, double y)
        {
            RequireScrollingFrame(name);
            lock (_lock)
            {
                Live(name)["ScrollPosition"] = new Vector3Data(x, y, 0);
            }
        }

        /// <summary>UIService.GetScrollPosition(Name)</summary>
        public Vector3Data? GetScrollPosition(string name)
        {
            RequireScrollingFrame(name);
            lock (_lock)
            {
                return Live(name).TryGetValue("ScrollPosition", out var value) && value is Vector3Data position
                    ? position
                    : null;
            }
        }

        /// <summary>Element view (tests/tooling).</summary>
        public UiElement? GetElement(string name)
        {
            lock (_lock)
            {
                return _elements.TryGetValue(name, out var element) ? element : null;
            }
        }

        /// <summary>Recorded tweens (engine consumption / tests).</summary>
        public IReadOnlyList<UiTween> GetTweens()
        {
            lock (_lock)
            {
                return _tweens.ToArray();
            }
        }

        /// <summary>Fires the click callbacks of an element (engine input / tests).</summary>
        public void FireClick(string name)
        {
            RequireName(name);
            Action[] callbacks;
            lock (_lock)
            {
                RequireElement(name);
                callbacks = (_clickCallbacks.TryGetValue(name, out var list) ? list : new List<Action>()).ToArray();
            }
            foreach (var callback in callbacks)
                callback();
        }

        /// <summary>Fires the text-changed callbacks of an element (engine input / tests).</summary>
        public void FireTextChanged(string name, string? newText)
        {
            RequireName(name);
            Action<string?>[] callbacks;
            lock (_lock)
            {
                RequireElement(name);
                Live(name)["Text"] = newText;
                callbacks = (_textChangedCallbacks.TryGetValue(name, out var list) ? list : new List<Action<string?>>()).ToArray();
            }
            foreach (var callback in callbacks)
                callback(newText);
        }

        private Dictionary<string, object?> Live(string name)
        {
            if (!_live.TryGetValue(name, out var live))
                _live[name] = live = new Dictionary<string, object?>();
            return live;
        }

        private void RequireElement(string name)
        {
            if (!_elements.ContainsKey(name))
                throw new InvalidOperationException($"No UI element '{name}'.");
        }

        private void RequireKind(string name, HashSet<string> kinds, string method)
        {
            RequireName(name);
            lock (_lock)
            {
                RequireElement(name);
                if (!kinds.Contains(_elements[name].Type))
                    throw new InvalidOperationException(
                        $"{method} does not apply to a '{_elements[name].Type}' element.");
            }
        }

        private void RequireScrollingFrame(string name) =>
            RequireKind(name, new HashSet<string> { "ScrollingFrame" }, "SetCanvasSize/GetCanvasSize/SetScrollPosition/GetScrollPosition");

        private static void AddCallback(Dictionary<string, List<Action>> map, string name, Action callback)
        {
            if (!map.TryGetValue(name, out var list))
                map[name] = list = new List<Action>();
            list.Add(callback);
        }

        private static string NormalizeType(string type) =>
            SupportedTypes.First(t => string.Equals(t, type, StringComparison.OrdinalIgnoreCase));

        private static void RequireName(string name)
        {
            if (string.IsNullOrWhiteSpace(name))
                throw new ArgumentException("Name must not be empty.", nameof(name));
        }

        private static IReadOnlyDictionary<string, object?> Copy(IReadOnlyDictionary<string, object?>? source) =>
            source == null
                ? new Dictionary<string, object?>()
                : new Dictionary<string, object?>(source);
    }

    /// <summary>
    /// InputService captures key, mouse and touch input events per player.
    /// The engine input pump calls the Fire* helpers; Luau-facing code only
    /// registers callbacks.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#inputservice
    /// </summary>
    public class InputService
    {
        /// <summary>Supported mouse buttons.</summary>
        public static readonly IReadOnlyList<string> MouseButtons =
            new[] { "Left", "Right", "Middle" };

        private sealed record InputKey(string Username, string Kind, string Code);

        private readonly object _lock = new();
        private readonly Dictionary<InputKey, List<Action>> _callbacks = new();
        private readonly Dictionary<string, (IReadOnlyList<string> Keys, Action Callback)> _actions = new(StringComparer.Ordinal);

        /// <summary>InputService.OnKeyPress(Username, KeyCode, Callback)</summary>
        public void OnKeyPress(string username, string keyCode, Action callback) =>
            Add(username, "KeyPress", RequireKeyCode(keyCode), callback);

        /// <summary>InputService.OnKeyRelease(Username, KeyCode, Callback)</summary>
        public void OnKeyRelease(string username, string keyCode, Action callback) =>
            Add(username, "KeyRelease", RequireKeyCode(keyCode), callback);

        /// <summary>InputService.OnMouseClick(Username, MouseButton, Callback)</summary>
        public void OnMouseClick(string username, string mouseButton, Action callback) =>
            Add(username, "MouseClick", RequireMouseButton(mouseButton), callback);

        /// <summary>InputService.OnMouseMove(Username, Callback)</summary>
        public void OnMouseMove(string username, Action callback) => Add(username, "MouseMove", "", callback);

        /// <summary>InputService.OnTouch(Username, Callback)</summary>
        public void OnTouch(string username, Action callback) => Add(username, "Touch", "", callback);

        /// <summary>InputService.BindAction(ActionName, Keys, Callback)</summary>
        public void BindAction(string actionName, IReadOnlyList<string> keys, Action callback)
        {
            if (string.IsNullOrWhiteSpace(actionName))
                throw new ArgumentException("ActionName must not be empty.", nameof(actionName));
            if (keys == null || keys.Count == 0)
                throw new ArgumentException("At least one key is required.", nameof(keys));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            foreach (string key in keys)
                RequireKeyCode(key);

            lock (_lock)
            {
                if (!_actions.TryAdd(actionName, (keys.ToArray(), callback)))
                    throw new InvalidOperationException($"Action '{actionName}' is already bound — UnbindAction first.");
            }
        }

        /// <summary>InputService.UnbindAction(ActionName)</summary>
        public void UnbindAction(string actionName)
        {
            if (string.IsNullOrWhiteSpace(actionName))
                throw new ArgumentException("ActionName must not be empty.", nameof(actionName));
            lock (_lock)
            {
                if (!_actions.Remove(actionName))
                    throw new InvalidOperationException($"No action bound as '{actionName}'.");
            }
        }

        /// <summary>Engine input pump: fires key press callbacks (tests included).</summary>
        public void FireKeyPress(string username, string keyCode) => Fire(username, "KeyPress", RequireKeyCode(keyCode));

        /// <summary>Engine input pump: fires key release callbacks.</summary>
        public void FireKeyRelease(string username, string keyCode) => Fire(username, "KeyRelease", RequireKeyCode(keyCode));

        /// <summary>Engine input pump: fires mouse click callbacks.</summary>
        public void FireMouseClick(string username, string mouseButton) =>
            Fire(username, "MouseClick", RequireMouseButton(mouseButton));

        /// <summary>Engine input pump: fires mouse move callbacks.</summary>
        public void FireMouseMove(string username) => Fire(username, "MouseMove", "");

        /// <summary>Engine input pump: fires touch callbacks.</summary>
        public void FireTouch(string username) => Fire(username, "Touch", "");

        /// <summary>Whether an action is bound.</summary>
        public bool IsBound(string actionName)
        {
            lock (_lock)
            {
                return _actions.ContainsKey(actionName);
            }
        }

        private void Add(string username, string kind, string code, Action callback)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));

            lock (_lock)
            {
                var key = new InputKey(username, kind, code);
                if (!_callbacks.TryGetValue(key, out var list))
                    _callbacks[key] = list = new List<Action>();
                list.Add(callback);
            }
        }

        private void Fire(string username, string kind, string code)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));

            Action[] callbacks;
            lock (_lock)
            {
                callbacks = _callbacks.TryGetValue(new InputKey(username, kind, code), out var list)
                    ? list.ToArray()
                    : Array.Empty<Action>();
            }
            foreach (var callback in callbacks)
                callback();
        }

        private string RequireKeyCode(string keyCode)
        {
            if (string.IsNullOrWhiteSpace(keyCode))
                throw new ArgumentException("KeyCode must not be empty.", nameof(keyCode));
            return keyCode;
        }

        private string RequireMouseButton(string mouseButton)
        {
            string? canonical = MouseButtons.FirstOrDefault(b =>
                string.Equals(b, mouseButton, StringComparison.OrdinalIgnoreCase));
            if (canonical == null)
                throw new ArgumentException(
                    $"MouseButton '{mouseButton}' is not supported. Allowed: {string.Join(", ", MouseButtons)}.",
                    nameof(mouseButton));
            return canonical;
        }
    }

    /// <summary>
    /// TextService processes, measures and formats text (measure, wrap,
    /// truncate, filter, escape). v0 measures with a fixed average glyph
    /// width; real glyph metrics arrive with the renderer integration.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#textservice
    /// </summary>
    public class TextService
    {
        /// <summary>Default font, per the docs.</summary>
        public const string DefaultFont = "Gotham";

        /// <summary>Default font size, per the docs.</summary>
        public const int DefaultFontSize = 14;

        /// <summary>Average glyph width factor used by v0 measurement.</summary>
        public const double GlyphWidthFactor = 0.6;

        /// <summary>Measured text size (MeasureText returns a {Width, Height} table).</summary>
        public sealed record TextMetrics(double Width, double Height);

        /// <summary>TextService.MeasureText(Text, Font, FontSize)</summary>
        public TextMetrics MeasureText(string text, string font = DefaultFont, double fontSize = DefaultFontSize)
        {
            RequireText(text);
            RequireFontSize(fontSize);
            return new TextMetrics(text.Length * fontSize * GlyphWidthFactor, fontSize * 1.2);
        }

        /// <summary>TextService.FitText(Text, MaxWidth, Font, FontSize) — true when the text fits one line.</summary>
        public bool FitText(string text, double maxWidth, string font = DefaultFont, double fontSize = DefaultFontSize)
        {
            if (maxWidth < 0)
                throw new ArgumentException("MaxWidth must not be negative.", nameof(maxWidth));
            return MeasureText(text, font, fontSize).Width <= maxWidth;
        }

        /// <summary>TextService.WrapText(Text, MaxWidth, Font, FontSize) — greedy word wrap.</summary>
        public IReadOnlyList<string> WrapText(string text, double maxWidth, string font = DefaultFont, double fontSize = DefaultFontSize)
        {
            if (maxWidth <= 0)
                throw new ArgumentException("MaxWidth must be greater than 0.", nameof(maxWidth));

            double charWidth = fontSize * GlyphWidthFactor;
            int charsPerLine = Math.Max(1, (int)(maxWidth / charWidth));
            var lines = new List<string>();
            foreach (string paragraph in RequireText(text).Split('\n'))
            {
                var words = paragraph.Split(' ');
                string current = "";
                foreach (string word in words)
                {
                    string candidate = current.Length == 0 ? word : current + " " + word;
                    if (candidate.Length <= charsPerLine)
                    {
                        current = candidate;
                    }
                    else
                    {
                        if (current.Length > 0)
                            lines.Add(current);
                        current = word.Length <= charsPerLine
                            ? word
                            : word.Substring(0, charsPerLine); // hard-break overlong words
                    }
                }
                lines.Add(current);
            }
            return lines;
        }

        /// <summary>TextService.Truncate(Text, MaxLines, Ellipsis) — keeps at most MaxLines lines.</summary>
        public string Truncate(string text, int maxLines, string ellipsis = "…")
        {
            if (maxLines < 1)
                throw new ArgumentException("MaxLines must be at least 1.", nameof(maxLines));
            if (ellipsis == null)
                ellipsis = "…";

            var lines = RequireText(text).Split('\n');
            if (lines.Length <= maxLines)
                return text;
            return string.Join('\n', lines.Take(maxLines)) + ellipsis;
        }

        /// <summary>
        /// TextService.Filter(Username, Text) — v0 returns the text
        /// unchanged: the word-filter settings live in ChatService, and the
        /// platform filter plugs in here later.
        /// </summary>
        public string Filter(string username, string text)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            return RequireText(text);
        }

        /// <summary>TextService.EscapeRichText(Text)</summary>
        public string EscapeRichText(string text)
        {
            RequireText(text);
            return text
                .Replace("&", "&amp;", StringComparison.Ordinal)
                .Replace("<", "&lt;", StringComparison.Ordinal)
                .Replace(">", "&gt;", StringComparison.Ordinal);
        }

        /// <summary>TextService.CountCharacters(Text)</summary>
        public int CountCharacters(string text) => RequireText(text).Length;

        private static string RequireText(string text)
        {
            if (text == null)
                throw new ArgumentNullException(nameof(text));
            return text;
        }

        private static void RequireFontSize(double fontSize)
        {
            if (fontSize <= 0)
                throw new ArgumentException("FontSize must be greater than 0.", nameof(fontSize));
        }
    }

    /// <summary>
    /// NotificationService shows Toast/Popup/Banner notifications to a player
    /// or everyone ("*"), with dismiss callbacks.
    ///
    /// See: https://noobietoria.github.io/Docs/en/api/#notificationservice
    /// </summary>
    public class NotificationService
    {
        /// <summary>Supported notification styles.</summary>
        public static readonly IReadOnlyList<string> Types =
            new[] { "Toast", "Popup", "Banner" };

        /// <summary>One notification (Send returns its auto-generated Id).</summary>
        public sealed record Notification(int NotificationId, string Username, string Title,
            string Message, string Type, double Duration);

        private int _nextId = 1;
        private readonly object _lock = new();
        private readonly List<Notification> _active = new();
        private readonly Dictionary<string, List<Action<int>>> _dismissCallbacks = new(StringComparer.Ordinal);

        /// <summary>NotificationService.Send(Username, Title, Message, Type, Duration)</summary>
        public int Send(string username, string title, string message,
            string type = "Toast", double duration = 5)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            var notification = Build(username, title, message, type, duration);
            lock (_lock)
            {
                _active.Add(notification);
                return notification.NotificationId;
            }
        }

        /// <summary>NotificationService.SendAll(Title, Message, Type, Duration)</summary>
        public int SendAll(string title, string message, string type = "Toast", double duration = 5)
        {
            var notification = Build("*", title, message, type, duration);
            lock (_lock)
            {
                _active.Add(notification);
                return notification.NotificationId;
            }
        }

        /// <summary>NotificationService.Dismiss(Username, NotificationId)</summary>
        public void Dismiss(string username, int notificationId)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));

            Action<int>[] callbacks;
            lock (_lock)
            {
                int removed = _active.RemoveAll(n =>
                    n.NotificationId == notificationId
                    && (n.Username == username || n.Username == "*" || username == "*"));
                if (removed == 0)
                    throw new InvalidOperationException(
                        $"No active notification {notificationId} for '{username}'.");
                callbacks = _dismissCallbacks.TryGetValue(username, out var list)
                    ? list.ToArray()
                    : Array.Empty<Action<int>>();
            }
            foreach (var callback in callbacks)
                callback(notificationId);
        }

        /// <summary>NotificationService.OnDismiss(Username, Callback) — receives the NotificationId.</summary>
        public void OnDismiss(string username, Action<int> callback)
        {
            if (string.IsNullOrWhiteSpace(username))
                throw new ArgumentException("Username must not be empty.", nameof(username));
            if (callback == null)
                throw new ArgumentNullException(nameof(callback));
            lock (_lock)
            {
                if (!_dismissCallbacks.TryGetValue(username, out var list))
                    _dismissCallbacks[username] = list = new List<Action<int>>();
                list.Add(callback);
            }
        }

        /// <summary>Active notifications for one player (tests/tooling).</summary>
        public IReadOnlyList<Notification> GetActive(string username)
        {
            lock (_lock)
            {
                return _active.Where(n => n.Username == username || n.Username == "*").ToArray();
            }
        }

        private Notification Build(string username, string title, string message, string type, double duration)
        {
            string? canonical = Types.FirstOrDefault(t =>
                string.Equals(t, type, StringComparison.OrdinalIgnoreCase));
            if (canonical == null)
                throw new ArgumentException(
                    $"Notification type '{type}' is not supported. Allowed: {string.Join(", ", Types)}.",
                    nameof(type));
            if (duration < 0)
                throw new ArgumentException("Duration must not be negative.", nameof(duration));

            return new Notification(_nextId++, username, title, message, canonical, duration);
        }
    }
}
