# Symfony bundle with portable PHP SDK

Generate from the repository root:

```sh
cargo run -p kaji-cli -- generate --config examples/symfony-sdk/kaji.json
```

The recipe creates example/notes-sdk and example/notes-symfony together. The
bundle's sdk_package explicitly references the generated portable SDK.

In a Symfony application's composer.json, add both output directories as
Composer path repositories, give the local path packages version 1.0.0 using repository options.versions,
then require example/notes-symfony:1.0.0. Enable
the generated NotesBundle in config/bundles.php. The generated bundle README
provides the exact configuration alias, service classes, and base URL settings.

Configure an actual API server and credentials before invoking operations.
The fixture contract contains GET /notes/{noteId}; this example does not run
a server. The bundle uses Symfony HttpClient through the portable SDK's PSR
transport interfaces.
