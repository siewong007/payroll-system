import babelParser from '@babel/eslint-parser'
import js from '@eslint/js'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import { defineConfig, globalIgnores } from 'eslint/config'

export default defineConfig([
  globalIgnores(['dist']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2020,
      globals: globals.browser,
      parser: babelParser,
      parserOptions: {
        requireConfigFile: false,
        babelOptions: {
          plugins: ['@babel/plugin-syntax-jsx'],
          presets: [['@babel/preset-typescript', { ignoreExtensions: true }]],
        },
      },
    },
    rules: {
      // TypeScript 7's compiler enforces these with noUnusedLocals,
      // noUnusedParameters, and strict mode. ESLint's core scope analyzer
      // treats type-only symbols as runtime JavaScript symbols.
      'no-undef': 'off',
      'no-unused-vars': 'off',
      'no-restricted-syntax': [
        'error',
        {
          selector: 'TSAnyKeyword',
          message: 'Use a specific type instead of any.',
        },
      ],
      'react-hooks/set-state-in-effect': 'off',
    },
  },
])
