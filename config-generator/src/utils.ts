function updateTheme() {
  const useDarkMode = window.matchMedia("(prefers-color-scheme: dark)").matches;

  window.document.documentElement.setAttribute(
    "data-theme",
    useDarkMode ? "dark" : "light"
  );
}

export { updateTheme };
