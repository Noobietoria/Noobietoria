// Noobietoria — GraphQL API client + shared helpers.
// The backend serves both the API (/graphql) and these static files, so the
// endpoint is same-origin by default. To point the UI at a separately hosted
// API, define window.NOOBIE_API_URL (e.g. "http://localhost:4000") before this
// script loads.

(function () {
    const API_URL = (window.NOOBIE_API_URL || "") + "/graphql";
    const TOKEN_KEY = "noob_token";

    const API = {
        get token() { return localStorage.getItem(TOKEN_KEY); },
        set token(value) {
            if (value) localStorage.setItem(TOKEN_KEY, value);
            else localStorage.removeItem(TOKEN_KEY);
        },

        async gql(query, variables = {}) {
            const res = await fetch(API_URL, {
                method: "POST",
                headers: {
                    "Content-Type": "application/json",
                    ...(API.token ? { Authorization: `Bearer ${API.token}` } : {}),
                },
                body: JSON.stringify({ query, variables }),
            });
            if (!res.ok) throw new Error(`API error ${res.status}`);
            const json = await res.json();
            if (json.errors && json.errors.length) {
                const err = new Error(json.errors[0].message);
                err.code = json.errors[0].extensions && json.errors[0].extensions.code;
                err.errors = json.errors;
                throw err;
            }
            return json.data;
        },

        /** Redirect to login if there is no session. Returns true when authed. */
        requireAuth() {
            if (!API.token) {
                window.location.href = "login.html";
                return false;
            }
            return true;
        },

        /** Fetch the logged-in user (or null). Small cache for layout use. */
        async me(force = false) {
            if (!API.token) return null;
            if (!force && API._me) return API._me;
            try {
                const data = await API.gql(`query { me {
                    id username email bio avatarEmoji bodyColorHue bannerColor level plusTier
                    credits coins streak longestStreak itemSales networth status createdAt lastSeen
                    friendsCount worldsCount
                } }`);
                API._me = data.me;
                return data.me;
            } catch {
                API.token = null;
                return null;
            }
        },

        async logout() {
            try { await API.gql(`mutation { logout }`); } catch { /* ignore */ }
            API.token = null;
            API._me = null;
            window.location.href = "login.html";
        },

        /* ---------- shared UI helpers ---------- */

        escapeHtml(s) {
            return String(s ?? "").replace(/[&<>"']/g, (c) => ({
                "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
            }[c]));
        },

        timeAgo(iso) {
            const sec = Math.max(1, Math.floor((Date.now() - new Date(iso).getTime()) / 1000));
            const units = [[31536000, "year"], [2592000, "month"], [604800, "week"],
                [86400, "day"], [3600, "hour"], [60, "minute"]];
            for (const [size, name] of units) {
                if (sec >= size) {
                    const n = Math.floor(sec / size);
                    return `${n} ${name}${n > 1 ? "s" : ""} ago`;
                }
            }
            return "just now";
        },

        formatDate(iso) {
            return new Date(iso).toLocaleDateString(undefined, { month: "short", year: "numeric" });
        },

        formatCount(n) {
            return Number(n || 0).toLocaleString("en-US");
        },

        /** Stable avatar accent class from a user id (av--1 … av--9). */
        avatarClass(id) {
            return "av--" + (1 + (Math.abs(Number(id) || 0) % 9));
        },

        initialOf(name) {
            return (name || "?").trim().charAt(0).toUpperCase();
        },

        statusDot(status) {
            if (status === "online") return 'style="background:var(--green);"';
            if (status === "ingame") return 'style="background:var(--blue);"';
            return 'style="border:2px solid var(--muted);width:7px;height:7px;"';
        },

        /** Toast message (bottom-right). kind: "info" | "error" */
        toast(message, kind = "info") {
            let host = document.getElementById("toast-host");
            if (!host) {
                host = document.createElement("div");
                host.id = "toast-host";
                document.body.appendChild(host);
            }
            const el = document.createElement("div");
            el.className = "toast" + (kind === "error" ? " toast--error" : "");
            el.textContent = message;
            host.appendChild(el);
            requestAnimationFrame(() => el.classList.add("is-visible"));
            setTimeout(() => {
                el.classList.remove("is-visible");
                setTimeout(() => el.remove(), 350);
            }, 3200);
        },

        /** Query-string helper (?u=username). */
        queryParam(name) {
            return new URLSearchParams(window.location.search).get(name);
        },
    };

    /* ---------- simple modal helper (window.NooModal) ---------- */
    const Modal = {
        open({ title, body, submitLabel = "Save", onSubmit }) {
            Modal.close();
            const overlay = document.createElement("div");
            overlay.className = "modal-overlay";
            overlay.id = "noo-modal";
            overlay.innerHTML = `
                <div class="modal card">
                    <h3 class="modal__title">${title}</h3>
                    <div class="modal__body">${body}</div>
                    <div class="modal__actions">
                        <button class="btn btn--outline" data-modal-cancel>Cancel</button>
                        <button class="btn btn--blue" data-modal-submit>${submitLabel}</button>
                    </div>
                </div>`;
            document.body.appendChild(overlay);

            const close = () => overlay.remove();
            overlay.addEventListener("click", (e) => {
                if (e.target === overlay) close();
            });
            overlay.querySelector("[data-modal-cancel]").addEventListener("click", close);

            const submitBtn = overlay.querySelector("[data-modal-submit]");
            submitBtn.addEventListener("click", async () => {
                submitBtn.disabled = true;
                try {
                    await onSubmit();
                    close();
                } catch (err) {
                    submitBtn.disabled = false;
                    window.API.toast(err.message, "error");
                }
            });

            const first = overlay.querySelector(".modal__input");
            if (first) first.focus();
        },
        close() {
            const existing = document.getElementById("noo-modal");
            if (existing) existing.remove();
        },
    };

    window.NooModal = Modal;

    window.API = API;
})();
