# contrib/ — prepared registry submissions

Ready-to-submit artifacts for getting codemmet into the editor-ecosystem
registries. Nothing in this directory is consumed by the plugin or the build;
each file is a copy prepared against the target registry's **current** schema
(verified 2026-07, against `mason-org/mason-registry` `packages/*/package.yaml`
and `neovim/nvim-lspconfig` `lsp/*.lua`), ready to be copied into a fork of
that registry and submitted as a PR.

When either upstream registry changes its schema, fix the copy here first,
then submit.

## mason-registry (`mason/codemmet/package.yaml`)

Registers codemmet with [mason.nvim](https://github.com/mason-org/mason.nvim)
so `:MasonInstall codemmet` downloads a release binary.

How to submit:

1. Fork <https://github.com/mason-org/mason-registry> and create a branch.
2. Copy `contrib/mason/codemmet/package.yaml` to `packages/codemmet/package.yaml`
   in the fork.
3. Sanity checks before opening the PR:
   - The `source.id` version (`@v0.4.1`) points at an existing GitHub release
     whose assets match the `file:` fields exactly. `{{version}}` interpolates
     to the git tag (`v0.4.1`), so the assets must be named
     `codemmet-v0.4.1-<target-triple>.{tar.gz,zip}` — which is what
     `.github/workflows/release.yml` produces.
   - The `neovim.lspconfig: codemmet` field requires the nvim-lspconfig
     submission below to be merged first (or drop the field from the initial
     PR and add it in a follow-up).
   - Run the registry's own validation from the fork root:
     `./scripts/inv validate` (see the registry's CONTRIBUTING.md; CI runs the
     same checks).
4. Open the PR against `mason-org/mason-registry` `main`. New-package PRs are
   largely automated; keep the PR to the single package file.

Maintenance: mason's Renovate bot bumps `source.id` to new tags automatically
once the package is merged, as long as future releases keep the same asset
naming scheme. If `release.yml` ever changes target triples or archive names,
the `asset:` list here must change with it.

Note the target coverage is deliberately the release matrix: `linux_x64_gnu`,
`darwin_arm64`, and `win_x64`. There is **no Intel-macOS asset** (dropped in
v0.4.1) — Intel Mac users install with `cargo install codemmet`, so no
`darwin_x64` entry.

## nvim-lspconfig (`lspconfig/codemmet.lua`)

Registers the server definition with
[nvim-lspconfig](https://github.com/neovim/nvim-lspconfig), whose current
scheme is one `lsp/<server>.lua` file per server returning a `vim.lsp.Config`
table (the legacy `lua/lspconfig/configs/*.lua` style is deprecated).

How to submit:

1. Fork <https://github.com/neovim/nvim-lspconfig> and create a branch.
2. Copy `contrib/lspconfig/codemmet.lua` to `lsp/codemmet.lua` in the fork.
3. Format with the registry's own stylua settings (`stylua lsp/codemmet.lua`
   from the fork root — nvim-lspconfig uses single quotes; this repo's stylua
   prefers double quotes, so expect the quotes to flip).
4. Run their checks per CONTRIBUTING.md (at minimum
   `make lint` / stylua + luacheck) and open the PR titled
   `feat: add codemmet`.

The definition matches the shipped plugin's defaults
(`lua/codemmet/init.lua`): `cmd = { "codemmet", "lsp" }`, the nine supported
filetypes, and `root_markers = { ".codemmet", ".git" }`.

Note for users, worth repeating in the PR description: the codemmet
repository ships its own Neovim plugin **and** a native `lsp/codemmet.lua`,
so Neovim users who install the plugin need neither this lspconfig entry nor
mason's; the lspconfig entry serves users who manage servers exclusively
through nvim-lspconfig/mason. The in-repo definition guards against
double-attach; this standalone copy intentionally stays plain (no guard) to
match lspconfig house style.
