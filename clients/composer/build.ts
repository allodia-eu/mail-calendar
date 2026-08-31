// Bundles the TypeScript editor sources into the single `editor.html` every client loads.
//
// The output is ONE self-contained file because that is what the four hosts can load: Apple's
// `loadHTMLString` and Linux's `include_str!` + `load_html` both hand WebKit a document with no
// real origin, so a `<script src>` (or an ES-module script, which carries origin semantics a
// classic script does not) has nothing to resolve against. Bundling to an IIFE sidesteps all of
// it: the sources stay ESM/TypeScript, and the artifact is the same inline `(() => {…})()` the
// editor has always shipped.
//
// The artifact is **committed**, unlike every other generated file in this repo. Not committing it
// would make bun a hard build dependency of the Rust workspace itself (`mailcal-linux` compiles the
// HTML in with `include_str!`, so it must exist at `cargo build` time), of MSBuild and of Gradle.
// `--check` buys back the guarantee that would otherwise be lost: it rebuilds and diffs, so a
// source edit that was never rebuilt fails the gate loudly instead of shipping stale.

// `import.meta.dir`, not `new URL(".", import.meta.url).pathname`: the latter yields `/C:/…` on
// Windows, which no file API here can open.
const ROOT = `${import.meta.dir}/`;
const OUTPUT = `${ROOT}dist/editor.html`;
const PLACEHOLDER = "<!--EDITOR_BUNDLE-->";

const BANNER = `<!--
  GENERATED FILE; do not edit.

  Source: clients/composer/src/ (TypeScript, ESM). Rebuild with:
      cd clients/composer && bun run build

  Committed deliberately: every client loads this one file (Apple copies it, Windows Content-includes
  it, Android reads it from assets, Linux include_str!s it), and generating it at build time would
  make bun a prerequisite of cargo, MSBuild and Gradle. "bun run check" re-derives it and fails if
  this copy is stale.
-->
`;

async function bundle(): Promise<string> {
  const built = await Bun.build({
    entrypoints: [`${ROOT}src/main.ts`],
    target: "browser",
    format: "iife",
    // Deliberately unminified: this file is committed and diffed by the gate, so a readable diff is
    // worth more than the bytes. It is a local asset; nothing downloads it.
    minify: false,
  });
  if (!built.success) {
    for (const log of built.logs) console.error(log);
    throw new Error("editor bundle failed to build");
  }
  const [artifact] = built.outputs;
  if (!artifact || built.outputs.length !== 1) {
    throw new Error(`expected exactly one bundle artifact, got ${built.outputs.length}`);
  }
  return (await artifact.text()).trimEnd();
}

async function render(): Promise<string> {
  const shell = await Bun.file(`${ROOT}src/index.html`).text();
  if (!shell.includes(PLACEHOLDER)) {
    throw new Error(`src/index.html is missing the ${PLACEHOLDER} placeholder`);
  }
  const script = `<script>\n${await bundle()}\n  </script>`;
  // A function replacer, not the string itself: `String.replace` reads `$&`, `` $` ``, `$'` and
  // `$1` in a *replacement string* as patterns, so a source file containing one would splice the
  // shell's own text into the bundle. `--check` would then agree with the corrupted artifact,
  // because it re-derives it the same way.
  return BANNER + shell.replace(PLACEHOLDER, () => script);
}

const rendered = await render();

if (Bun.argv.includes("--check")) {
  const existing = await Bun.file(OUTPUT)
    .text()
    .catch(() => "");
  if (existing !== rendered) {
    console.error(
      "clients/composer/dist/editor.html is stale: it does not match src/.\n" +
        "Rebuild it with:  cd clients/composer && bun run build",
    );
    process.exit(1);
  }
  console.log("OK: clients/composer/dist/editor.html matches src/.");
} else {
  await Bun.write(OUTPUT, rendered);
  console.log(`wrote editor.html (${rendered.length} bytes)`);
}
