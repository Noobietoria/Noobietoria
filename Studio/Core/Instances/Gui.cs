using System;

namespace Noobietoria.Studio.Core
{
    /// <summary>
    /// Base class for the documented GUI Instances (Frame, TextLabel,
    /// TextButton, ImageLabel, ImageButton, TextBox, ScrollingFrame).
    ///
    /// See: https://noobietoria.github.io/Docs/en/instance/#frame
    /// </summary>
    public abstract class GuiObject : Instance
    {
        protected GuiObject(string name, string type, string rootContainer)
            : base(name, type, rootContainer)
        {
        }

        /// <summary>Tint of the element (RGBA channels in the 0..1 range).</summary>
        public ColorData? Color
        {
            get => Properties.TryGetValue("RGBA", out var value) && value is ColorData color
                ? color
                : null;
            set => SetProperty("RGBA", value);
        }
    }

    /// <summary>Plain GUI panel.</summary>
    public class Frame : GuiObject
    {
        public Frame(string name, string rootContainer)
            : base(name, "Frame", rootContainer)
        {
        }
    }

    /// <summary>Base for text-bearing GUI elements (TextLabel, TextButton, TextBox).</summary>
    public abstract class TextGuiObject : GuiObject
    {
        protected TextGuiObject(string name, string type, string rootContainer)
            : base(name, type, rootContainer)
        {
        }

        public string? Text
        {
            get => GetProperty<string?>("Text");
            set => SetProperty("Text", value);
        }

        public double? TextSize
        {
            get => Properties.TryGetValue("TextSize", out var value) && value is double size
                ? size
                : null;
            set => SetProperty("TextSize", value);
        }

        public ColorData? TextColor3
        {
            get => Properties.TryGetValue("TextColor3", out var value) && value is ColorData color
                ? color
                : null;
            set => SetProperty("TextColor3", value);
        }
    }

    /// <summary>Displays text.</summary>
    public class TextLabel : TextGuiObject
    {
        public TextLabel(string name, string rootContainer)
            : base(name, "TextLabel", rootContainer)
        {
        }
    }

    /// <summary>Clickable text element.</summary>
    public class TextButton : TextGuiObject
    {
        public TextButton(string name, string rootContainer)
            : base(name, "TextButton", rootContainer)
        {
        }
    }

    /// <summary>Editable text field.</summary>
    public class TextBox : TextGuiObject
    {
        public TextBox(string name, string rootContainer)
            : base(name, "TextBox", rootContainer)
        {
        }

        public string? PlaceholderText
        {
            get => GetProperty<string?>("PlaceholderText");
            set => SetProperty("PlaceholderText", value);
        }
    }

    /// <summary>Base for image-bearing GUI elements (ImageLabel, ImageButton).</summary>
    public abstract class ImageGuiObject : GuiObject
    {
        protected ImageGuiObject(string name, string type, string rootContainer)
            : base(name, type, rootContainer)
        {
        }

        /// <summary>Image asset id from NImageAssets.</summary>
        public string? Image
        {
            get => GetProperty<string?>("Image");
            set => SetProperty("Image", value);
        }

        public double? ImageTransparency
        {
            get => Properties.TryGetValue("ImageTransparency", out var value) && value is double transparency
                ? transparency
                : null;
            set => SetProperty("ImageTransparency", value);
        }
    }

    /// <summary>Displays an image.</summary>
    public class ImageLabel : ImageGuiObject
    {
        public ImageLabel(string name, string rootContainer)
            : base(name, "ImageLabel", rootContainer)
        {
        }
    }

    /// <summary>Clickable image element.</summary>
    public class ImageButton : ImageGuiObject
    {
        public ImageButton(string name, string rootContainer)
            : base(name, "ImageButton", rootContainer)
        {
        }
    }

    /// <summary>Frame with scrolling content.</summary>
    public class ScrollingFrame : GuiObject
    {
        public ScrollingFrame(string name, string rootContainer)
            : base(name, "ScrollingFrame", rootContainer)
        {
        }

        public bool? ScrollingEnabled
        {
            get => Properties.TryGetValue("ScrollingEnabled", out var value) && value is bool enabled
                ? enabled
                : null;
            set => SetProperty("ScrollingEnabled", value);
        }
    }
}
