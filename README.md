# sline-transpiler

Handlebars to Sline converter for SHOPLINE themes. It helps migrate Online Store 2.1 themes (Handlebars) to Online Store 3.0 (Sline) by rewriting the most common template constructs into Sline syntax and flagging unsupported patterns.

## Why this eases 2.1 → 3.0 migration

- Converts the core Handlebars control flow (`#each`, `#if`, `#unless`) into Sline `#for` and `#if` tags.
- Normalizes scope helpers like `this` and `./` to the Sline data model.
- Preserves existing comments by converting Handlebars block comments into Sline comment blocks.
- Warns when Handlebars features cannot be mapped directly, so you know exactly what to fix by hand.

## Install

### From source

```bash
cargo build --release
./target/release/sline-transpiler --help
```

### Homebrew

After you publish a release tarball and set the SHA in your tap formula:

```bash
brew tap admirsaheta/homebrew-handlesline
brew install sline-transpiler
```

## Usage

Convert a file and print to stdout:

```bash
sline-transpiler path/to/template.hbs
```

Write output to a file:

```bash
sline-transpiler path/to/template.hbs -o path/to/template.sline
```

Read from stdin:

```bash
cat path/to/template.hbs | sline-transpiler --stdin
```

Fail CI if conversion finds unsupported features:

```bash
sline-transpiler path/to/template.hbs --check
```

Strip parent scope `../` references and continue:

```bash
sline-transpiler path/to/template.hbs --allow-parent
```

## What it converts

### Handlebars → Sline control flow

```hbs
{{#each products as |product|}}
  {{ product.title }}
{{/each}}
```

```sline
{{#for product in products}}
  {{ product.title }}
{{/for}}
```

```hbs
{{#if featured}}
  Featured
{{else}}
  Standard
{{/if}}
```

```sline
{{#if featured}}
  Featured
{{else}}
  Standard
{{/if}}
```

```hbs
{{#unless items.size > 0}}
  Empty
{{/unless}}
```

```sline
{{#if !(items.size > 0)}}
  Empty
{{/if}}
```

### Scope normalization

```hbs
{{this.title}}
{{./price}}
```

```sline
{{title}}
{{price}}
```

Inside `#each`, `this` becomes the alias:

```hbs
{{#each products as |product|}}
  {{this.title}}
{{/each}}
```

```sline
{{#for product in products}}
  {{product.title}}
{{/for}}
```

### Comments

```hbs
{{#comment}}
Block comment
{{/comment}}
```

```sline
{{!--
Block comment
--}}
```

## Limitations

- `../` parent scope access is not supported in Sline. Use `--allow-parent` to strip it and keep going.
- Handlebars helpers are not automatically mapped to Sline filters. These need manual updates.
- Nested block features outside of `#each`, `#if`, `#unless`, and `#comment` are left as-is with warnings.

## Development

```bash
cargo check
cargo clippy -- -D warnings
```
