let currentPublicUrl = "";

async function loadDashboard() {
  const main = document.getElementById("main");
  try {
    const [me, payload, blocks] = await Promise.all([
      api("/api/auth/me"),
      api("/api/pages/me"),
      api("/api/pages/me/blocks"),
    ]);
    const page = payload.page;
    document.getElementById("welcome").textContent = `${I18N[currentLang()].welcome}، ${me.username}`;
    document.getElementById("welcome-line").textContent =
      page.status === "published" ? I18N[currentLang()].published : I18N[currentLang()].draft;
    if (me.role === "admin") {
      document.getElementById("admin-link").hidden = false;
    }
    const linkCount = blocks.filter((block) => (block.block_type || block.type) === "link").length;
    currentPublicUrl = `${location.origin}${payload.public_path}`;
    document.getElementById("profile-overview-title").textContent = page.display_name;
    document.getElementById("profile-initial").textContent =
      Array.from(page.display_name.trim())[0] || "L";
    document.getElementById("profile-bio").textContent =
      page.bio || I18N[currentLang()].profileBioPlaceholder;
    document.getElementById("public-url").textContent = currentPublicUrl;
    document.getElementById("public-url-link").href =
      page.status === "published" ? payload.public_path : "/preview";
    document.getElementById("live-page-link").href =
      page.status === "published" ? payload.public_path : "/preview";
    const statusPill = document.getElementById("profile-status");
    statusPill.classList.toggle("is-published", page.status === "published");
    statusPill.querySelector("span:last-child").textContent =
      page.status === "published" ? I18N[currentLang()].published : I18N[currentLang()].draft;
    document.getElementById("btn-publish").disabled = page.status === "published";
    document.getElementById("btn-unpublish").disabled = page.status !== "published";
    document.getElementById("stats").innerHTML = `
      <article class="stat"><span class="muted">${I18N[currentLang()].statLinks}</span><b>${linkCount}</b></article>
      <article class="stat"><span class="muted">${I18N[currentLang()].statBlocks}</span><b>${payload.block_count}</b></article>
      <article class="stat"><span class="muted">${I18N[currentLang()].statTheme}</span><b class="stat-text">${escapeHtml(page.theme)}</b></article>
      <article class="stat"><span class="muted">${I18N[currentLang()].statCreated}</span><b class="stat-text">${escapeHtml(page.created_at.slice(0, 10))}</b></article>
    `;
    document.getElementById("links-empty").hidden = linkCount !== 0;
    const form = document.getElementById("page-form");
    form.display_name.value = page.display_name;
    form.bio.value = page.bio || "";
    form.avatar_url.value = page.avatar_url || "";
    form.cover_url.value = page.cover_url || "";
    form.theme.value = page.theme;
    form.accent_color.value = page.accent_color;
    form.seo_title.value = page.seo_title || "";
    form.seo_description.value = page.seo_description || "";
    document.getElementById("account-meta").textContent =
      `${me.email} · ${me.username} · ${me.created_at.slice(0, 10)}`;
    document.getElementById("new_email").value = me.email;
    applyI18n();
  } catch (err) {
    if (err.status === 401) {
      location.href = "/login";
      return;
    }
    toast(err.message, "err");
  } finally {
    hideAppLoader();
  }
}

document.getElementById("page-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const form = event.target;
  const error = form.querySelector(".form-error");
  error.textContent = "";
  try {
    await api("/api/pages/me", {
      method: "PATCH",
      body: JSON.stringify({
        display_name: form.display_name.value,
        bio: form.bio.value,
        avatar_url: form.avatar_url.value,
        cover_url: form.cover_url.value,
        theme: form.theme.value,
        accent_color: form.accent_color.value,
        seo_title: form.seo_title.value,
        seo_description: form.seo_description.value,
      }),
    });
    toast("ذخیره شد", "ok");
    await loadDashboard();
  } catch (err) {
    error.textContent = err.message;
  }
});

document.getElementById("btn-publish").addEventListener("click", async () => {
  try {
    await api("/api/pages/me/publish", { method: "POST" });
    toast("صفحه منتشر شد", "ok");
    await loadDashboard();
  } catch (err) {
    toast(err.message, "err");
  }
});

document.getElementById("btn-unpublish").addEventListener("click", async () => {
  try {
    await api("/api/pages/me/unpublish", { method: "POST" });
    toast("انتشار لغو شد", "ok");
    await loadDashboard();
  } catch (err) {
    toast(err.message, "err");
  }
});

document.getElementById("btn-copy").addEventListener("click", async () => {
  try {
    if (!currentPublicUrl) {
      const payload = await api("/api/pages/me");
      currentPublicUrl = `${location.origin}${payload.public_path}`;
    }
    await navigator.clipboard.writeText(currentPublicUrl);
    toast(I18N[currentLang()].urlCopied, "ok");
  } catch (err) {
    toast(err.message || I18N[currentLang()].copyFailed, "err");
  }
});

document.getElementById("email-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const form = event.target;
  try {
    await api("/api/auth/email", {
      method: "PATCH",
      body: JSON.stringify({ email: form.email.value, password: form.password.value }),
    });
    toast("ایمیل به‌روز شد", "ok");
    form.password.value = "";
    await loadDashboard();
  } catch (err) {
    toast(err.message, "err");
  }
});

document.getElementById("password-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const form = event.target;
  try {
    await api("/api/auth/change-password", {
      method: "POST",
      body: JSON.stringify({
        current_password: form.current_password.value,
        new_password: form.new_password.value,
        new_password_confirmation: form.new_password_confirmation.value,
      }),
    });
    toast("رمز تغییر کرد. دوباره وارد شوید.", "ok");
    location.href = "/login";
  } catch (err) {
    toast(err.message, "err");
  }
});

document.addEventListener("DOMContentLoaded", loadDashboard);
