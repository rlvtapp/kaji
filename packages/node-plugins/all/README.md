# @relevate/kaji-plugins

A bundle of Kaji's native SDK and compiled Rust auxiliary plugin factories.
Install with `@relevate/kaji` and add only the plugins you want to
`kaji.config.mjs`. Installation does not register any renderer.

Language exports: `pluginTypeScript`, `pluginRust`, `pluginGo`, `pluginPython`, `pluginPhp`, `pluginJava`, `pluginCSharp`, `pluginElixir`, `pluginRuby`, `pluginSwift`.

Auxiliary exports: `pluginZod`, `pluginFaker`, `pluginMsw`, `pluginCypress`, `pluginReactQuery`, `pluginVueQuery`, `pluginSwr`.

Input exports: inputGraphql, inputAsyncApi, inputArazzo, inputProtobuf, inputCapnProto.

For a smaller install, use individual `@relevate/kaji-plugin-<name>` packages
instead. These factories select implementations compiled into Kaji's addon.
