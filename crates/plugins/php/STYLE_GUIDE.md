# PHP SDK client styles

Select the style on the SDK plugin:

```rust
use poolster::{php, prelude::*};

let package = php::package("php")
    .with(php::sdk().namespaced()); // Default; use .flat() for direct methods.
```

Namespaced clients use `$client->contacts()->get(...)`; flat clients use
`$client->getContact(...)`.
Direct operation methods also remain available on namespaced clients.
Actual signatures depend on the operation's declared parameters and body.

The generated package's own `STYLE_GUIDE.md` records the selected surface.
