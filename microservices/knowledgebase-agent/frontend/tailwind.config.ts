import type { Config } from "tailwindcss";
import typography from "@tailwindcss/typography";

const config: Config = {
  important: "#ms-mount",
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        surface: "#f0f0f3",
        card: "#ffffff",
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "sans-serif"],
      },
    },
  },
  plugins: [typography],
};

export default config;
