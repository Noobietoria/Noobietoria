// Noobietoria — character page: avatar preview, body colors, catalog with equip/buy.
document.addEventListener("DOMContentLoaded", async () => {
    if (!window.API || !window.API.requireAuth()) return;

    const preview = document.querySelector(".avatar-preview");
    const swatches = document.querySelectorAll(".color-swatch");
    const searchInput = document.querySelector(".item-search");
    const tabs = document.querySelectorAll('[data-tabs="catalog"] .item-tab');
    const panels = document.querySelectorAll('[data-tab-panel="catalog"]');
    if (!preview) return;

    const CATEGORY_ENUM = { accessories: "accessory", clothing: "clothing", tools: "tool", faces: "face", bodies: "body", outfits: "outfit" };
    const PAGE_SIZE = 18;
    let currentTab = "all";
    let pages = {}; // tab -> {page, total}

    function renderPreview(me, equipped) {
        preview.textContent = me.avatarEmoji;
        preview.style.filter = `hue-rotate(${me.bodyColorHue || 0}deg)`;
        preview.title = equipped.map((e) => e.item.name).join(", ") || "No items equipped";
    }

    const itemCard = (item, equippedIds) => {
        const isEquipped = equippedIds.has(item.id);
        let action;
        if (isEquipped) {
            action = `<button class="btn btn--green btn--sm" data-unequip="${item.id}">Equipped ✓</button>`;
        } else if (item.owned && item.slot) {
            action = `<button class="btn btn--outline btn--sm" data-equip="${item.id}">Equip</button>`;
        } else if (item.owned) {
            action = `<span class="section-eyebrow">Owned</span>`;
        } else if (item.price > 0) {
            action = `<button class="btn btn--blue btn--sm" data-buy="${item.id}">${item.currency === "coins" ? "$" : "▣"} ${item.price}</button>`;
        } else {
            action = `<button class="btn btn--blue btn--sm" data-buy="${item.id}">Free</button>`;
        }
        return `
            <figure class="catalog-item">
                <div class="thumb" ${item.color ? `style="background:${item.color};"` : ""}>${item.emoji}</div>
                <figcaption>
                    <div class="catalog-item__name">${window.API.escapeHtml(item.name)}</div>
                    <div class="catalog-item__by">${window.API.escapeHtml(item.category.replace("_", " "))}</div>
                    <div style="margin-top:8px;">${action}</div>
                </figcaption>
            </figure>`;
    };

    async function loadTab(tab) {
        const panel = document.querySelector(`[data-tab-panel="catalog"][data-panel-id="${tab}"]`);
        if (!panel) return;
        const state = pages[tab] || (pages[tab] = { page: 1 });
        const category = tab === "all" ? null : CATEGORY_ENUM[tab];

        const data = await window.API.gql(`query($c: ItemCategory, $s: String, $p: Int) {
            me { avatarEmoji bodyColorHue avatar { equipped { item { id } } } }
            catalog(category: $c, search: $s, page: $p, pageSize: ${PAGE_SIZE}) {
                total items { id name emoji color category slot price currency owned }
            }
        }`, { c: category, s: searchInput ? searchInput.value : "", p: state.page });

        renderPreview(data.me, data.me.avatar.equipped);
        const equippedIds = new Set(data.me.avatar.equipped.map((e) => e.item.id));
        state.total = data.catalog.total;

        const pageCount = Math.max(1, Math.ceil(data.catalog.total / PAGE_SIZE));
        panel.innerHTML = `
            <div class="catalog-grid">
                ${data.catalog.items.map((i) => itemCard(i, equippedIds)).join("")}
            </div>
            <div class="pagination">
                <button class="page-btn" data-nav="prev" ${state.page <= 1 ? "disabled" : ""}>«</button>
                ${Array.from({ length: pageCount }, (_, i) =>
                    `<button class="page-btn${i + 1 === state.page ? " is-active" : ""}" data-nav="${i + 1}">${i + 1}</button>`).join("")}
                <button class="page-btn" data-nav="next" ${state.page >= pageCount ? "disabled" : ""}>»</button>
            </div>`;

        panel.querySelectorAll("[data-equip]").forEach((btn) => btn.addEventListener("click", () =>
            equipToggle(`mutation($id: ID!){ equipItem(itemId: $id){ equipped { item { id } } } }`, btn.dataset.equip, tab)));
        panel.querySelectorAll("[data-unequip]").forEach((btn) => btn.addEventListener("click", () =>
            equipToggle(`mutation($id: ID!){ unequipItem(itemId: $id){ equipped { item { id } } } }`, btn.dataset.unequip, tab)));
        panel.querySelectorAll("[data-buy]").forEach((btn) => btn.addEventListener("click", async () => {
            btn.disabled = true;
            try {
                await window.API.gql(`mutation($id: ID!){ buyItem(itemId: $id){ item { name } } }`, { id: btn.dataset.buy });
                window.API.toast("Item purchased!");
                pages = {};
                loadTab(tab);
            } catch (err) {
                window.API.toast(err.message, "error");
                btn.disabled = false;
            }
        }));
        panel.querySelectorAll(".page-btn[data-nav]").forEach((btn) => btn.addEventListener("click", () => {
            const v = btn.dataset.nav;
            if (v === "prev" && state.page > 1) state.page--;
            else if (v === "next" && state.page < pageCount) state.page++;
            else if (v !== "prev" && v !== "next") state.page = Number(v);
            loadTab(tab);
        }));
    }

    async function equipToggle(mutation, itemId, tab) {
        try {
            await window.API.gql(mutation, { id: itemId });
            loadTab(tab);
        } catch (err) {
            window.API.toast(err.message, "error");
        }
    }

    // tabs
    tabs.forEach((tab) => {
        tab.addEventListener("click", () => {
            tabs.forEach((t) => t.classList.remove("is-active"));
            tab.classList.add("is-active");
            currentTab = tab.dataset.tab;
            panels.forEach((p) => { p.style.display = p.dataset.panelId === currentTab ? "" : "none"; });
            loadTab(currentTab);
        });
    });

    // search
    let timer = null;
    if (searchInput) {
        searchInput.addEventListener("input", () => {
            clearTimeout(timer);
            timer = setTimeout(() => loadTab(currentTab), 300);
        });
    }

    // body colors — persist to the backend
    swatches.forEach((swatch) => {
        swatch.addEventListener("click", async () => {
            swatches.forEach((s) => s.classList.remove("is-active"));
            swatch.classList.add("is-active");
            const hue = Number(swatch.dataset.hue || 0);
            preview.style.filter = `hue-rotate(${hue}deg)`;
            try {
                await window.API.gql(`mutation($h: Int!){ setBodyColor(hue: $h){ bodyColorHue } }`, { h: hue });
            } catch (err) {
                window.API.toast(err.message, "error");
            }
        });
    });

    // initial: load "all" tab, other panels load when first opened
    loadTab("all");
    panels.forEach((p) => {
        if (p.dataset.panelId !== "all") p.innerHTML = '<p class="section-eyebrow">Loading…</p>';
    });
});
