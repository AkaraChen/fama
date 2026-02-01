# Fama Test Project

This is a test project for verifying all fama formatters work correctly.

## Test Files

Located in `src/test-files/`:

- `test.js` - Unformatted JavaScript
- `test.jsx` - Unformatted JSX
- `test.ts` - Unformatted TypeScript
- `test.tsx` - Unformatted TSX
- `test.css` - Unformatted CSS
- `test.scss` - Unformatted SCSS
- `test.less` - Unformatted LESS
- `test.html` - Unformatted HTML
- `test.vue` - Unformatted Vue SFC
- `test.svelte` - Unformatted Svelte
- `test.astro` - Unformatted Astro
- `test.yaml` - Unformatted YAML
- `test.md` - Unformatted Markdown

## Running Fama

From the fama directory:

```bash
cd /Users/akrc/Developer/fama
cargo run --release -- format ../test-project/
```

Or from this project directory:

```bash
cargo run --release --manifest-path=../fama/Cargo.toml format .
```

## Verification

After running fama, check that all test files in `src/test-files/` are properly formatted with:

- Consistent indentation
- Lines under 80 characters (where applicable)
- No trailing whitespace
- Consistent quote style

## Development

```bash
npm install    # Install dependencies
npm run dev    # Start dev server
npm run build  # Build for production
npm run preview  # Preview production build
```
