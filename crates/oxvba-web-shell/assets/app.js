const shellState = {
  bridgeContractVersion: "v1",
  screens: ["workspace", "editor", "diagnostics", "immediate", "debugger"],
  activeScreen: "workspace",
};

function showScreen(screenName) {
  if (!shellState.screens.includes(screenName)) {
    return;
  }
  shellState.activeScreen = screenName;
  document.querySelectorAll("[data-screen]").forEach((screen) => {
    screen.classList.toggle("is-active", screen.dataset.screen === screenName);
  });
  document.querySelectorAll("[data-screen-target]").forEach((tab) => {
    const isActive = tab.dataset.screenTarget === screenName;
    tab.classList.toggle("is-active", isActive);
    tab.setAttribute("aria-current", isActive ? "page" : "false");
  });
  window.location.hash = screenName;
}

document.querySelectorAll("[data-screen-target]").forEach((tab) => {
  tab.addEventListener("click", () => showScreen(tab.dataset.screenTarget));
});

document.querySelectorAll("[data-command]").forEach((button) => {
  button.addEventListener("click", () => {
    const state = document.querySelector("[data-run-state]");
    if (!state) {
      return;
    }
    if (button.dataset.command === "run") {
      state.textContent = "Completed";
    } else if (button.dataset.command === "reset") {
      state.textContent = "Idle";
    } else {
      state.textContent = "Workspace chooser";
    }
  });
});

const initialScreen = window.location.hash.replace("#", "");
showScreen(shellState.screens.includes(initialScreen) ? initialScreen : shellState.activeScreen);
console.log("OxIde frankentui shell", shellState);
