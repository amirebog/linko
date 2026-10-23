let page = 1;
let query = "";

async function loadAdmin() {
  try {
    const me = await api("/api/auth/me");
    if (me.role !== "admin") {
      location.href = "/dashboard";
      return;
    }
    const stats = await api("/api/admin/stats");
    document.getElementById("stats").innerHTML = `
      <article class="stat"><span class="muted">کاربران</span><b>${stats.users}</b></article>
      <article class="stat"><span class="muted">صفحات</span><b>${stats.pages}</b></article>
      <article class="stat"><span class="muted">منتشرشده</span><b>${stats.published_pages}</b></article>
      <article class="stat"><span class="muted">لغو انتشار / پیش‌نویس</span><b>${stats.unpublished_pages + stats.draft_pages}</b></article>
    `;
    const users = await api(`/api/admin/users?page=${page}&per_page=10&q=${encodeURIComponent(query)}`);
    const body = document.getElementById("users");
    body.innerHTML = users.users.map((u) => `
      <tr>
        <td>${escapeHtml(u.username)}</td>
        <td>${escapeHtml(u.email)}</td>
        <td>${escapeHtml(u.role)}</td>
        <td>${escapeHtml(u.account_status)}</td>
        <td>${escapeHtml(u.page_status || "-")} / ${escapeHtml(u.page_slug || "")}</td>
        <td>
          <button class="btn btn-ghost" data-status="${u.account_status === "active" ? "disabled" : "active"}" data-id="${u.id}">
            ${u.account_status === "active" ? "غیرفعال" : "فعال"}
          </button>
          <button class="btn btn-danger" data-del="${u.id}">حذف</button>
        </td>
      </tr>
    `).join("");
    const pages = Math.max(1, Math.ceil(users.total / users.per_page));
    document.getElementById("pager").innerHTML = `
      <button class="btn btn-ghost" type="button" ${page <= 1 ? "disabled" : ""} data-page="${page - 1}">قبلی</button>
      <span class="muted">${page} / ${pages}</span>
      <button class="btn btn-ghost" type="button" ${page >= pages ? "disabled" : ""} data-page="${page + 1}">بعدی</button>
    `;
    const audit = await api("/api/admin/audit");
    document.getElementById("audit").innerHTML = audit.map((row) =>
      `<li><code>${escapeHtml(row.created_at)}</code> ${escapeHtml(row.action)} → ${escapeHtml(row.target_type)} ${escapeHtml(row.target_id || "")}</li>`
    ).join("") || "<li class='muted'>هنوز اقدامی ثبت نشده است.</li>";
  } catch (err) {
    if (err.status === 401) location.href = "/login";
    else if (err.status === 403) location.href = "/dashboard";
    else toast(err.message, "err");
  } finally {
    hideAppLoader();
  }
}

document.getElementById("search-form").addEventListener("submit", (event) => {
  event.preventDefault();
  query = event.target.q.value.trim();
  page = 1;
  loadAdmin();
});

document.getElementById("pager").addEventListener("click", (event) => {
  const btn = event.target.closest("[data-page]");
  if (!btn || btn.disabled) return;
  page = Number(btn.dataset.page);
  loadAdmin();
});

document.getElementById("users").addEventListener("click", async (event) => {
  const statusBtn = event.target.closest("[data-status]");
  const delBtn = event.target.closest("[data-del]");
  try {
    if (statusBtn) {
      await api(`/api/admin/users/${statusBtn.dataset.id}/status`, {
        method: "PATCH",
        body: JSON.stringify({ status: statusBtn.dataset.status }),
      });
      toast("وضعیت به‌روز شد", "ok");
    }
    if (delBtn) {
      if (!(await confirmAction("این حساب و داده‌های مرتبط حذف شوند؟"))) return;
      await api(`/api/admin/users/${delBtn.dataset.del}`, { method: "DELETE" });
      toast("حساب حذف شد", "ok");
    }
    await loadAdmin();
  } catch (err) {
    toast(err.message, "err");
  }
});

document.addEventListener("DOMContentLoaded", loadAdmin);
