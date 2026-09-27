# Java SDK client styles

Select the style on the SDK plugin:

```rust
use kaji::{java, prelude::*};

let package = java::package("java")
    .with(java::sdk().namespaced()); // Default; use .flat() for direct methods.
```

Namespaced clients use `client.contacts().get(input)`; flat clients use
`client.getContact(input)`.
Direct operation methods also remain available on namespaced clients.
Actual signatures depend on the operation's declared parameters and body.

The generated package's own `STYLE_GUIDE.md` records the selected surface.
