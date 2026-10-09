# Check and write output

← [Rust SDK](README.md)

Generation returns an owned `GeneratedTree` before anything is written.

```rust
let files = poolster::generate_native(profiles)?;
let changes = files.check("generated")?;
println!("{changes:?}");
files.write_to("generated")?;
```

Use the same root for checking and writing. Generation does not install package
dependencies, compile clients or publish releases.

Compile and exercise the generated package before shipping it. File ownership
protects unrelated files and detects locally edited generated files.

**Next:** [File ownership](../internals/files.md) · [Verification reference](../verification/verification.md)
