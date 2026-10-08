# @relevate/poolster-plugins

A bundle of Poolster's native SDK and compiled Rust auxiliary plugin factories.
Install with `@relevate/poolster` and add only the plugins you want to
`poolster.config.mjs`. Installation does not register any renderer.

Language exports: `pluginTypeScript`, `pluginRust`, `pluginGo`, `pluginPython`, `pluginPhp`, `pluginJava`, `pluginCSharp`, `pluginElixir`, `pluginRuby`, `pluginSwift`.

Auxiliary exports: `pluginZod`, `pluginFaker`, `pluginMsw`, `pluginCypress`, `pluginReactQuery`, `pluginVueQuery`, `pluginSwr`.

Input exports: inputGraphql, inputAsyncApi, inputArazzo, inputProtobuf, inputCapnProto.

For a smaller install, use individual `@relevate/poolster-plugin-<name>` packages
instead. These factories select implementations compiled into Poolster's addon.
