document.addEventListener("click", (event) => {
  const button = event.target.closest("[data-password-toggle]");
  if (!button) return;

  const field = button.closest(".password-field");
  const input = field?.querySelector("input");
  if (!input || !field.contains(button) || !["password", "text"].includes(input.type)) return;

  const visible = input.type === "password";
  input.type = visible ? "text" : "password";
  button.setAttribute("aria-pressed", String(visible));
  button.setAttribute(
    "data-i18n-aria-label",
    visible ? "hidePassword" : "showPassword",
  );
  const language = document.documentElement.lang === "en" ? "en" : "fa";
  button.setAttribute("aria-label", I18N[language][visible ? "hidePassword" : "showPassword"]);
  const showIcon = button.querySelector('[data-password-icon="show"]');
  const hideIcon = button.querySelector('[data-password-icon="hide"]');
  if (showIcon && hideIcon) {
    showIcon.hidden = visible;
    hideIcon.hidden = !visible;
  }
});
