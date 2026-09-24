/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{vue,js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        // 深色应用主色调（Postman 风格），色值定义在 style.css :root 的 CSS 变量中
        app: {
          bg: "rgb(var(--app-bg) / <alpha-value>)",
          surface: "rgb(var(--app-surface) / <alpha-value>)",
          surface2: "rgb(var(--app-surface2) / <alpha-value>)",
          border: "rgb(var(--app-border) / <alpha-value>)",
          hover: "rgb(var(--app-hover) / <alpha-value>)",
          active: "rgb(var(--app-active) / <alpha-value>)",
          text: "rgb(var(--app-text) / <alpha-value>)",
          muted: "rgb(var(--app-muted) / <alpha-value>)",
        },
        accent: {
          orange: "rgb(var(--accent-orange) / <alpha-value>)",
          "orange-hover": "rgb(var(--accent-orange-hover) / <alpha-value>)",
          blue: "rgb(var(--accent-blue) / <alpha-value>)",
          green: "rgb(var(--accent-green) / <alpha-value>)",
          red: "rgb(var(--accent-red) / <alpha-value>)",
          yellow: "rgb(var(--accent-yellow) / <alpha-value>)",
          purple: "rgb(var(--accent-purple) / <alpha-value>)",
        },
      },
      fontFamily: {
        mono: ["'JetBrains Mono'", "Menlo", "Consolas", "monospace"],
      },
    },
  },
  plugins: [],
};
