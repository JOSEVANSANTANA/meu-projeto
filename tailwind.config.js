/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        mono: ["'JetBrains Mono'", "'IBM Plex Mono'", "Consolas", "monospace"],
      },
      colors: {
        terminal: {
          bg: "#0a0e14",
          panel: "#11161f",
          border: "#1f2733",
          amber: "#ffb000",
          green: "#00d26a",
          red: "#ff3b3b",
          dim: "#8b98a9",
        },
      },
    },
  },
  plugins: [],
};
