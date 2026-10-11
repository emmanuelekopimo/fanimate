# Lucide icons

SVG files from [lucide-static](https://www.npmjs.com/package/lucide-static) **1.48.0**, unchanged, under the ISC licence in [LICENSE](LICENSE).

To add an icon, download it from the same version so the set stays consistent:

```sh
curl -o name.svg https://cdn.jsdelivr.net/npm/lucide-static@1.48.0/icons/name.svg
```

Then add a variant to `Icon` in `src/icons.rs`, map it in `Icon::svg`, and add it to the icon test. Browse names at <https://lucide.dev/icons>.
