/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{vue,ts}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        surface: {
          DEFAULT: "#0f1117",
          raised: "#161923",
          overlay: "#1c2030",
          border: "#262b3d",
        },
        accent: {
          DEFAULT: "#6d7cff",
          hover: "#828fff",
          soft: "rgba(109, 124, 255, 0.14)",
        },
        ok: "#3ecf8e",
        warn: "#f5b851",
        danger: "#f56e6e",
      },
      fontFamily: {
        sans: [
          "Inter",
          "Segoe UI Variable",
          "Segoe UI",
          "PingFang SC",
          "Microsoft YaHei UI",
          "Microsoft YaHei",
          "system-ui",
          "sans-serif",
        ],
        mono: [
          "JetBrains Mono",
          "Cascadia Code",
          "Consolas",
          "ui-monospace",
          "monospace",
        ],
      },
    },
  },
  plugins: [],
};
