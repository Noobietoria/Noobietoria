// Noobietoria — colorblind mode (shared across all pages).
// This is the same simulation approach originally built into landing.html:
// SVG feColorMatrix filters applied to <body> via a class, selected from a
// small topbar dropdown, persisted in localStorage.
(function () {
    var STORAGE_KEY = "noobietoria-cb-mode";
    var MODES = ["protanopia", "deuteranopia", "tritanopia", "achromatopsia"];
    var LABELS = {
        none: "Default",
        protanopia: "Protanopia (red-weak)",
        deuteranopia: "Deuteranopia (green-weak)",
        tritanopia: "Tritanopia (blue-weak)",
        achromatopsia: "Achromatopsia (no color)"
    };

    var FILTER_DEFS_ID = "cb-filter-defs";
    var FILTER_SVG = '' +
        '<svg id="' + FILTER_DEFS_ID + '" style="position:absolute;width:0;height:0" aria-hidden="true" focusable="false">' +
            '<defs>' +
                '<filter id="cb-protanopia">' +
                    '<feColorMatrix type="matrix" values="' +
                        '0.567 0.433 0     0 0 ' +
                        '0.558 0.442 0     0 0 ' +
                        '0     0.242 0.758 0 0 ' +
                        '0     0     0     1 0"/>' +
                '</filter>' +
                '<filter id="cb-deuteranopia">' +
                    '<feColorMatrix type="matrix" values="' +
                        '0.625 0.375 0   0 0 ' +
                        '0.7   0.3   0   0 0 ' +
                        '0     0.3   0.7 0 0 ' +
                        '0     0     0   1 0"/>' +
                '</filter>' +
                '<filter id="cb-tritanopia">' +
                    '<feColorMatrix type="matrix" values="' +
                        '0.95  0.05  0     0 0 ' +
                        '0     0.433 0.567 0 0 ' +
                        '0     0.475 0.525 0 0 ' +
                        '0     0     0     1 0"/>' +
                '</filter>' +
                '<filter id="cb-achromatopsia">' +
                    '<feColorMatrix type="matrix" values="' +
                        '0.299 0.587 0.114 0 0 ' +
                        '0.299 0.587 0.114 0 0 ' +
                        '0.299 0.587 0.114 0 0 ' +
                        '0     0     0     1 0"/>' +
                '</filter>' +
            '</defs>' +
        '</svg>';

    function ensureFilterDefs() {
        if (document.getElementById(FILTER_DEFS_ID)) return;
        document.body.insertAdjacentHTML("afterbegin", FILTER_SVG);
    }

    function getSavedMode() {
        try {
            return localStorage.getItem(STORAGE_KEY) || "none";
        } catch (e) {
            return "none";
        }
    }

    function applyMode(mode) {
        var body = document.body;
        MODES.forEach(function (m) { body.classList.remove("cb-" + m); });
        if (mode && mode !== "none") {
            body.classList.add("cb-" + mode);
        }
        document.querySelectorAll(".cb-switcher__menu button[data-mode]").forEach(function (btn) {
            btn.classList.toggle("is-active", btn.dataset.mode === mode);
        });
    }

    function setMode(mode) {
        try { localStorage.setItem(STORAGE_KEY, mode); } catch (e) { /* ignore */ }
        applyMode(mode);
        window.dispatchEvent(new CustomEvent("colorblindmodechange", { detail: { mode: mode } }));
    }

    window.Noobietoria = window.Noobietoria || {};
    window.Noobietoria.colorblind = { setMode: setMode, getMode: getSavedMode, MODES: MODES };

    var eyeIcon = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M1 12s4-7 11-7 11 7 11 7-4 7-11 7-11-7-11-7z"/><circle cx="12" cy="12" r="3"/></svg>';

    function buildSwitcher() {
        var current = getSavedMode();
        var items = ["none"].concat(MODES).map(function (mode) {
            var active = mode === current ? ' class="is-active"' : "";
            return '<li role="none"><button role="menuitem" data-mode="' + mode + '"' + active + '>' + LABELS[mode] + '</button></li>';
        }).join("");

        var wrap = document.createElement("div");
        wrap.className = "cb-switcher";
        wrap.innerHTML =
            '<button class="cb-switcher__btn" type="button" aria-haspopup="true" aria-expanded="false" title="Colorblind mode">' +
                eyeIcon +
            '</button>' +
            '<ul class="cb-switcher__menu" role="menu">' + items + '</ul>';

        var toggle = wrap.querySelector(".cb-switcher__btn");
        var menu = wrap.querySelector(".cb-switcher__menu");

        toggle.addEventListener("click", function (e) {
            e.stopPropagation();
            var isOpen = menu.classList.toggle("is-open");
            toggle.setAttribute("aria-expanded", isOpen ? "true" : "false");
            if (isOpen) {
                var activeBtn = menu.querySelector("button.is-active") || menu.querySelector("button[data-mode]");
                if (activeBtn) activeBtn.focus();
            }
        });

        menu.addEventListener("click", function (e) {
            var btn = e.target.closest("button[data-mode]");
            if (!btn) return;
            setMode(btn.dataset.mode);
            menu.classList.remove("is-open");
            toggle.setAttribute("aria-expanded", "false");
        });

        document.addEventListener("click", function () {
            menu.classList.remove("is-open");
            toggle.setAttribute("aria-expanded", "false");
        });

        var items = menu.querySelectorAll("button[data-mode]");

        menu.addEventListener("keydown", function (e) {
            var idx = Array.prototype.indexOf.call(items, document.activeElement);
            if (e.key === "ArrowDown") {
                e.preventDefault();
                items[(idx + 1) % items.length].focus();
            } else if (e.key === "ArrowUp") {
                e.preventDefault();
                items[(idx - 1 + items.length) % items.length].focus();
            } else if (e.key === "Home") {
                e.preventDefault();
                items[0].focus();
            } else if (e.key === "End") {
                e.preventDefault();
                items[items.length - 1].focus();
            }
        });

        document.addEventListener("click", function () {
            menu.classList.remove("is-open");
            toggle.setAttribute("aria-expanded", "false");
        });

        document.addEventListener("keydown", function (e) {
            if (e.key === "Escape" && menu.classList.contains("is-open")) {
                menu.classList.remove("is-open");
                toggle.setAttribute("aria-expanded", "false");
                toggle.focus();
            }
        });

        window.addEventListener("colorblindmodechange", function (e) {
            var label = LABELS[e.detail.mode] || LABELS.none;
            toggle.setAttribute("aria-label", "Colorblind mode: " + label);
        });
        toggle.setAttribute("aria-label", "Colorblind mode: " + (LABELS[current] || LABELS.none));

        return wrap;
    }

    function mountSwitcher() {
        if (document.querySelector(".cb-switcher")) return true;

        // App shell pages (home, profile, worlds, ...): topbar built by layout.js
        var topbarRight = document.querySelector(".topbar__right");
        if (topbarRight) {
            var notifBtn = topbarRight.querySelector('[title="Notifications"]');
            var switcher = buildSwitcher();
            if (notifBtn) topbarRight.insertBefore(switcher, notifBtn);
            else topbarRight.appendChild(switcher);
            return true;
        }

        // Landing page navbar: mount just before the auth links.
        var navLinks = document.querySelector(".navbar__links");
        if (navLinks) {
            navLinks.insertBefore(buildSwitcher(), navLinks.firstChild);
            return true;
        }

        return false;
    }

    document.addEventListener("DOMContentLoaded", function () {
        ensureFilterDefs();
        applyMode(getSavedMode());

        if (!mountSwitcher()) {
            // topbar is injected slightly after DOMContentLoaded by layout.js in some cases
            var tries = 0;
            var timer = setInterval(function () {
                tries++;
                if (mountSwitcher() || tries > 20) clearInterval(timer);
            }, 50);
        }
    });
})();