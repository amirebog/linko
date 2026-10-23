async function onRegister(event) {
  event.preventDefault();
  const form = event.target;
  const error = form.querySelector(".form-error");
  const submit = form.querySelector('[type="submit"]');
  error.textContent = "";
  setAuthFormLoading(form, submit, true);
  try {
    await api("/api/auth/register", {
      method: "POST",
      body: JSON.stringify({
        username: form.username.value,
        email: form.email.value,
        password: form.password.value,
        password_confirmation: form.password_confirmation.value,
      }),
    });
    location.href = "/dashboard";
  } catch (err) {
    error.textContent = err.message;
  } finally {
    setAuthFormLoading(form, submit, false);
  }
}

async function onLogin(event) {
  event.preventDefault();
  const form = event.target;
  const error = form.querySelector(".form-error");
  const submit = form.querySelector('[type="submit"]');
  error.textContent = "";
  setAuthFormLoading(form, submit, true);
  try {
    await api("/api/auth/login", {
      method: "POST",
      body: JSON.stringify({
        identifier: form.identifier.value,
        password: form.password.value,
      }),
    });
    location.href = "/dashboard";
  } catch (err) {
    error.textContent = err.message;
  } finally {
    setAuthFormLoading(form, submit, false);
  }
}

function setAuthFormLoading(form, submit, loading) {
  form.setAttribute("aria-busy", String(loading));
  submit.disabled = loading;
  submit.classList.toggle("is-loading", loading);
}

const registerForm = document.getElementById("register-form");
if (registerForm) registerForm.addEventListener("submit", onRegister);

const loginForm = document.getElementById("login-form");
if (loginForm) loginForm.addEventListener("submit", onLogin);
