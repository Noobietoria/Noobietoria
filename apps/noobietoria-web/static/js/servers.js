// Noobietoria — server directory client.
//
// The website is the single source of truth for join endpoints: pages ask this
// module for the servers of a place instead of asking the player for an IP
// address and a port.
//
// REST surface (see apps/noobietoria-web/src/site.rs):
//   GET /api/v1/servers                       every joinable server
//   GET /api/v1/places/{place_id}/servers     joinable servers of one place
//
// Each entry looks like:
//   { server_id, place_id, place_name, host, port,
//     players, max_players, region, ping_ms, status }
(function () {
    var API_BASE = window.NOOBIE_API_URL || "";

    function esc(value) {
        return String(value == null ? "" : value).replace(/[&<>"']/g, function (c) {
            return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c];
        });
    }

    var Servers = {
        escape: esc,

        /** Every joinable server of one place. Resolves to an array. */
        placeServers: async function (placeId) {
            var res = await fetch(
                API_BASE + "/api/v1/places/" + encodeURIComponent(placeId) + "/servers"
            );
            if (!res.ok) throw new Error("API error " + res.status);
            var json = await res.json();
            return json.servers || [];
        },

        /** Every joinable server on the platform. Resolves to an array. */
        allServers: async function () {
            var res = await fetch(API_BASE + "/api/v1/servers");
            if (!res.ok) throw new Error("API error " + res.status);
            var json = await res.json();
            return json.servers || [];
        },

        /** Occupancy label, e.g. "12/50". */
        occupancy: function (server) {
            return String(server.players) + "/" + String(server.max_players);
        },

        /** Endpoint label, e.g. "127.0.0.1:53640". */
        endpoint: function (server) {
            return String(server.host) + ":" + String(server.port);
        },

        /** A full/"offline" server cannot be joined. */
        isJoinable: function (server) {
            return server.status !== "full" && server.status !== "offline";
        },

        /** Status pill markup. */
        statusPill: function (server) {
            var state = server.status === "full" ? "full"
                : server.status === "offline" ? "offline" : "online";
            var label = state === "full" ? "Full" : state === "offline" ? "Offline" : "Online";
            return '<span class="srv-status srv-status--' + state + '">' + label + "</span>";
        },

        /** HTML for one server row inside the join dialog. */
        rowHtml: function (server) {
            var joinable = Servers.isJoinable(server);
            var ping = server.ping_ms ? esc(server.ping_ms) + " ms" : "—";
            return (
                '<div class="srv-row" data-server-id="' + esc(server.server_id) + '">' +
                    '<div class="srv-row__main">' +
                        '<span class="srv-row__region">' + esc(server.region || "Unknown region") + "</span>" +
                        '<span class="srv-row__id">#' + esc(server.server_id) + "</span>" +
                    "</div>" +
                    '<div class="srv-row__stats">' +
                        "<span>👤 " + esc(Servers.occupancy(server)) + "</span>" +
                        "<span>🏓 " + ping + "</span>" +
                        Servers.statusPill(server) +
                    "</div>" +
                    '<div class="srv-row__action">' +
                        '<code class="srv-row__endpoint">' + esc(Servers.endpoint(server)) + "</code>" +
                        '<button class="btn btn--green btn--sm" data-join-server="' + esc(server.server_id) + '"' +
                            (joinable ? "" : " disabled") + ">▶ Join</button>" +
                    "</div>" +
                "</div>"
            );
        },

        /**
         * Opens the join dialog for a world: lists its servers and lets the
         * player pick one. `onJoin(server)` is called with the chosen server.
         */
        openJoinDialog: async function (placeId, placeName, onJoin) {
            var overlay = document.createElement("div");
            overlay.className = "modal-overlay";
            overlay.id = "noo-server-modal";
            overlay.innerHTML =
                '<div class="modal card modal--wide">' +
                    '<h3 class="modal__title">' + esc(placeName || placeId) + "</h3>" +
                    '<p class="srv-hint">Choose a server to join. Endpoints come from the Noobietoria API — no IP address to type.</p>' +
                    '<div class="srv-list" id="srv-list"><p class="srv-loading">Loading servers…</p></div>' +
                    '<div class="modal__actions">' +
                        '<button class="btn btn--outline" data-modal-cancel>Close</button>' +
                        '<button class="btn btn--outline" data-srv-reload>⟳ Refresh</button>' +
                    "</div>" +
                "</div>";
            document.body.appendChild(overlay);

            var listEl = overlay.querySelector("#srv-list");

            function close() {
                overlay.remove();
            }

            overlay.addEventListener("click", function (e) {
                if (e.target === overlay) close();
            });
            overlay.querySelector("[data-modal-cancel]").addEventListener("click", close);

            async function load() {
                listEl.innerHTML = '<p class="srv-loading">Loading servers…</p>';
                try {
                    var servers = await Servers.placeServers(placeId);
                    if (!servers.length) {
                        listEl.innerHTML =
                            '<p class="srv-empty">No server is running this place right now.</p>';
                        return;
                    }
                    listEl.innerHTML = servers.map(Servers.rowHtml).join("");
                    listEl.querySelectorAll("[data-join-server]").forEach(function (btn) {
                        btn.addEventListener("click", function () {
                            var id = btn.getAttribute("data-join-server");
                            var server = servers.filter(function (s) {
                                return String(s.server_id) === String(id);
                            })[0];
                            if (!server) return;
                            if (typeof onJoin === "function") onJoin(server);
                            close();
                        });
                    });
                } catch (err) {
                    listEl.innerHTML =
                        '<p class="srv-empty">Could not load servers: ' + esc(err.message) + "</p>";
                }
            }

            overlay.querySelector("[data-srv-reload]").addEventListener("click", load);
            await load();
        },
    };

    window.NooServers = Servers;
})();
