import js from "@eslint/js"
import reactHooks from "eslint-plugin-react-hooks"
import reactRefresh from "eslint-plugin-react-refresh"
import tseslint from "typescript-eslint"

export default tseslint.config(
  { ignores: ["dist"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    // `astryx theme build` uses this reference to load generated variant types.
    files: ["src/theme/astryx*.d.ts"],
    rules: {
      "@typescript-eslint/triple-slash-reference": "off",
    },
  },
  {
    files: ["**/*.{ts,tsx}"],
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "react-refresh/only-export-components": ["warn", { allowConstantExport: true }],
    },
  },
  {
    files: ["src/stories/components/**/*.{ts,tsx}"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: ["*.css", "**/*.css"],
              message: "Astryx component stories must compose Astryx primitives instead of authored CSS.",
            },
          ],
        },
      ],
      "no-restricted-syntax": [
        "error",
        ...[
          "article",
          "div",
          "h1",
          "h2",
          "h3",
          "h4",
          "h5",
          "h6",
          "header",
          "main",
          "p",
          "section",
          "span",
          "svg",
        ].map((element) => ({
          selector: `JSXOpeningElement[name.name='${element}']`,
          message: `Use an Astryx primitive instead of a story-authored <${element}>.`,
        })),
      ],
    },
  },
)
