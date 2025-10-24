# Releasing

1. Update version in `Cargo.toml`
2. Commit the change
3. Create and push a tag:
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

GitHub Actions will automatically test and publish to crates.io.
