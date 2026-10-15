// Noobietoria — shared layout: injects topbar + footer into every page.
// Pages set <body data-page="...">; active items are highlighted via [data-nav].
// When logged in, the topbar shows the user's real currencies + avatar (via GraphQL).
(function () {
    var ICONS = {
        globe: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c2.5 2.6 3.9 5.7 3.9 9s-1.4 6.4-3.9 9c-2.5-2.6-3.9-5.7-3.9-9S9.5 5.6 12 3z"/></svg>',
        cart: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="9" cy="21" r="1.6"/><circle cx="19" cy="21" r="1.6"/><path d="M2 3h3l2.7 12.4a2 2 0 0 0 2 1.6h9.7a2 2 0 0 0 2-1.6L23 7H6"/></svg>',
        shield: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-3.6 8-10V5l-8-3-8 3v7c0 6.4 8 10 8 10z"/></svg>',
        users: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>',
        chat: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>',
        search: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/></svg>',
        bell: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 8a6 6 0 0 0-12 0c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.7 21a2 2 0 0 1-3.4 0"/></svg>'
    };

    var SUBNAV = [
        ["home", "Home", "home.html"],
        ["character", "Character", "character.html"],
        ["inventory", "Inventory", "inventory.html"],
        ["create", "Create", "create.html"],
        ["friends", "Friends", "people.html"],
        ["inbox", "Inbox", "#"],
        ["trades", "Trades", "#"],
        ["rankings", "Rankings", "#"]
    ];

    function buildHeader() {
        var page = document.body.dataset.page || "";
        var subnavLinks = SUBNAV.map(function (item) {
            var active = item[0] === page ? ' class="subnav__link is-active"' : ' class="subnav__link"';
            return '<a' + active + ' data-nav="' + item[0] + '" href="' + item[2] + '">' + item[1] + '</a>';
        }).join("");

        return '' +
        '<header class="topbar">' +
            '<div class="topbar__inner">' +
                '<a class="brand" href="home.html"><img src="./vector/logo.svg" alt="Noobietoria logo"></a>' +
                '<nav class="topbar__nav">' +
                    '<a class="topnav__link" href="worlds.html">' + ICONS.globe + 'Worlds <span class="count-pill" data-worlds-count>38</span></a>' +
                    '<a class="topnav__link" href="character.html">' + ICONS.cart + 'Store</a>' +
                    '<a class="topnav__link" href="guilds.html">' + ICONS.shield + 'Guilds</a>' +
                    '<a class="topnav__link" href="people.html">' + ICONS.users + 'People</a>' +
                    '<a class="topnav__link" href="#">' + ICONS.chat + 'Forum</a>' +
                '</nav>' +
                '<div class="topbar__search">' + ICONS.search + '<input type="text" placeholder="Search Noobietoria..."></div>' +
                '<div class="topbar__right">' +
                    '<div class="currency" title="Credits"><span class="currency__icon currency__icon--cube">\u25A3</span><span data-currency="credits">31</span></div>' +
                    '<div class="currency" title="Coins"><span class="currency__icon currency__icon--coin">$</span><span data-currency="coins">299</span></div>' +
                    '<div class="currency" title="Streak"><span class="currency__icon currency__icon--flame">\uD83D\uDD25</span><span data-currency="streak">29</span></div>' +
                    '<button class="icon-btn" title="Notifications">' + ICONS.bell + '<span class="icon-btn__dot"></span></button>' +
                    '<a class="topbar__avatar" href="profile.html" title="abcsonpoly" data-topbar-avatar>A</a>' +
                    '<button class="icon-btn" title="Log out" data-logout>\u23FB</button>' +
                '</div>' +
            '</div>' +
            '<nav class="subnav"><div class="subnav__inner">' + subnavLinks +
                '<a class="subnav__upgrade" href="#">\u2726 Upgrade</a>' +
            '</div></nav>' +
        '</header>';
    }

    function buildFooter() {
        return '' +
        '<footer class="footer"><div class="footer__grid">' +
            '<div class="footer__brand">' +
                '<img src="./vector/logo.svg" alt="Noobietoria logo">' +
                '<div class="footer__version">web v0.1.0<br>\u00A9 2023 - 2026 Noobietoria</div>' +
            '</div>' +
            '<div><h4>Noobietoria</h4><ul>' +
                '<li><a href="home.html">Home</a></li>' +
                '<li><a href="worlds.html">Worlds</a></li>' +
                '<li><a href="character.html">Store</a></li>' +
                '<li><a href="#">Forum</a></li>' +
                '<li><a href="people.html">People</a></li>' +
            '</ul></div>' +
            '<div><h4>About</h4><ul>' +
                '<li><a href="#">Support</a></li>' +
                '<li><a href="#">Blog</a></li>' +
                '<li><a href="#">Privacy Policy</a></li>' +
                '<li><a href="#">Terms of Service</a></li>' +
                '<li><a href="#">Noobietoria Rules</a></li>' +
            '</ul></div>' +
            '<div><h4>Social</h4><ul>' +
                '<li><a href="#">Discord</a></li>' +
                '<li><a href="#">GitHub</a></li>' +
                '<li><a href="#">YouTube</a></li>' +
                '<li><a href="#">X (Twitter)</a></li>' +
            '</ul></div>' +
        '</div></footer>';
    }

    document.addEventListener("DOMContentLoaded", function () {
        document.body.insertAdjacentHTML("afterbegin", buildHeader());
        document.body.insertAdjacentHTML("beforeend", buildFooter());

        // ----- populate the topbar with real session data -----
        if (window.API) {
            var logoutBtn = document.querySelector("[data-logout]");
            if (logoutBtn) {
                logoutBtn.addEventListener("click", function () {
                    if (window.API.token) window.API.logout();
                    else window.location.href = "login.html";
                });
            }

            window.API.me().then(function (me) {
                if (!me) return;
                var set = function (sel, value) {
                    var el = document.querySelector(sel);
                    if (el) el.textContent = value;
                };
                set('[data-currency="credits"]', window.API.formatCount(me.credits));
                set('[data-currency="coins"]', window.API.formatCount(me.coins));
                set('[data-currency="streak"]', me.streak);
                var avatar = document.querySelector("[data-topbar-avatar]");
                if (avatar) {
                    avatar.textContent = window.API.initialOf(me.username);
                    avatar.title = me.username;
                }
            });

            // worlds count pill
            window.API.gql("query { worlds { id } }").then(function (data) {
                var pill = document.querySelector("[data-worlds-count]");
                if (pill) pill.textContent = data.worlds.length;
            }).catch(function () { /* leave the placeholder */ });
        }
    });
})();