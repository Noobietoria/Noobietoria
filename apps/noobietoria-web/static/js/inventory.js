// Noobietoria — inventory page: category filter + paginated owned items.
document.addEventListener("DOMContentLoaded", async () => {
    if (!window.API || !window.API.requireAuth()) return;

    const filters = document.querySelectorAll(".inv-filter");
    const grid = document.querySelector(".catalog-grid");
    const pagination = document.querySelector(".pagination");
    const title = document.querySelector(".inventory-title");
    const breadcrumbUser = document.querySelector(".breadcrumb a[href='profile.html']");
    if (!grid) return;

    const CATEGORIES = ["hat", "tool", "face", "clothing", "game_pass", "achievement", "body", "theme"];
    const PAGE_SIZE = 24;
    let category = null; // null = everything
    let page = 1;

    async function load() {
        const data = await window.API.gql(`query($c: ItemCategory, $p: Int) {
            me { username }
            inventory(category: $c, page: $p, pageSize: ${PAGE_SIZE}) {
                total entries { item { id name emoji color category creator { username } } }
            }
        }`, { c: category, p: page });

        if (title) title.textContent = `${data.me.username}'s inventory`;
        if (breadcrumbUser) {
            breadcrumbUser.textContent = data.me.username;
            breadcrumbUser.href = "profile.html";
        }

        const { entries, total } = data.inventory;
        grid.innerHTML = entries.length ? entries.map((e) => `
            <figure class="catalog-item">
                <div class="thumb" ${e.item.color ? `style="background:${e.item.color};"` : ""}>${e.item.emoji}</div>
                <figcaption>
                    <div class="catalog-item__name">${window.API.escapeHtml(e.item.name)}</div>
                    <div class="catalog-item__by">by ${window.API.escapeHtml(e.item.creator ? e.item.creator.username : "Noobietoria")}</div>
                </figcaption>
            </figure>`).join("")
            : '<p class="section-eyebrow">No items in this category yet.</p>';

        const pageCount = Math.max(1, Math.ceil(total / PAGE_SIZE));
        if (pagination) {
            let html = `<button class="page-btn" data-page-nav="prev" ${page <= 1 ? "disabled" : ""}>«</button>`;
            for (let i = 1; i <= pageCount; i++) {
                html += `<button class="page-btn${i === page ? " is-active" : ""}" data-page-nav="${i}">${i}</button>`;
            }
            html += `<button class="page-btn" data-page-nav="next" ${page >= pageCount ? "disabled" : ""}>»</button>`;
            pagination.innerHTML = html;
            pagination.querySelectorAll("[data-page-nav]").forEach((btn) => {
                btn.addEventListener("click", () => {
                    const v = btn.dataset.pageNav;
                    if (v === "prev" && page > 1) page--;
                    else if (v === "next" && page < pageCount) page++;
                    else if (v !== "prev" && v !== "next") page = Number(v);
                    load();
                });
            });
        }
    }

    filters.forEach((filter, i) => {
        filter.addEventListener("click", (e) => {
            e.preventDefault();
            filters.forEach((f) => f.classList.remove("is-active"));
            filter.classList.add("is-active");
            category = i === 0 ? null : CATEGORIES[i - 1] || null;
            page = 1;
            load();
        });
    });

    load();
});
