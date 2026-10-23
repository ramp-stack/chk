# CHK Overview

CHK is a Rust application-building layer designed to sit on top of UI systems built with **Prism**. Rather than requiring developers to construct user interfaces manually, CHK allows builders to describe the **data they want to collect or display**, and automatically generates the corresponding application interface.

Instead of choosing specific UI components such as text inputs, radio selectors, or tables, developers simply define the information a user should interact with. CHK then determines how that information should be presented within the underlying UI system.

CHK currently targets **Pelican UI**, with plans to support additional Prism-based UI systems in the future.

## Design Goals

CHK prioritizes **intent and simplicity** over manual UI construction. The system is designed to reduce the amount of UI code developers need to write by allowing them to focus on application logic and data rather than component selection and layout decisions.

Applications built with CHK are typically defined in terms of:

* **Displays** for presenting information
* **Forms** for collecting user input
* **Roots** for defining the main sections of an application

From these definitions, CHK constructs the appropriate pages, navigation, and input interfaces automatically.

This approach allows developers to build structured applications quickly without needing to understand the details of the underlying UI framework.

## Scope

CHK provides:

* A declarative system for defining application flows and forms
* Automatic generation of pages and navigation
* Structured displays for presenting data
* A consistent interface layer built on top of Prism UI systems

CHK intentionally avoids exposing low-level UI components to the builder. Instead, it focuses on describing **what the application should do**, leaving the UI implementation details to the framework.

## Relationship to UI Systems

CHK operates above the Prism ecosystem and generates interfaces for supported UI systems.

```text
Application
    ↓
CHK
    ↓
UI System (Pelican UI)
    ↓
Prism
    ↓
wgpu
```

Because CHK operates at the application-description layer, it can support multiple Prism-based UI frameworks without requiring developers to change how their application logic is defined.

## Refreshing pages

`Page::watching` subscribes to Air changes through Prism's `OnChange`. `Watch::contract`
watches one instance; `Watch::list` watches both new instances and changes to existing
members. Tuples combine sources, for example a chat room and the block list.

`Listener::new` receives navigation information before building its initial page, so
construction builds once. Subsequent notifications rebuild the page with the same
navigation and preserve message drafts. Notifications coalesce while a page is hidden;
reactivation reloads its snapshot before consuming a potentially lagged queue. Idle polls do not clone or compare contract
snapshots, and watched contracts no longer need `PartialEq`.

`Listener` remains the owner of the built page and its builder. Custom `PageBuilder`
implementations supply `ChangeSource<Update = ()>` and register the supplied waker when
pending. A cloned watched page refreshes once on activation to reconcile changes that
preceded its new subscriptions.

## Validators with changing dependencies

Ordinary closures remain valid form validators. When validity also depends on a
change source, pass `Validator::watch(source, callback)` instead. The form checks
initially, after input edits, and when the source wakes it; no `TickEvent`, manual
refresh, or page rebuild is needed.

The callback receives `&Watched<S>`. It dereferences to the source, so Air instance
methods such as `pending()` and player methods such as `frame()` remain available.
For a source that delivers values only through updates, `latest()` returns the
last update, or `None` before the first update. The last update remains available
after the source completes and when the user edits an input again.

For example, a source delivering available amounts as `u64` can validate a number:

```rust
use chk::{Context, FormItem, FormValidState, NumberVariant, Validator, Watched};
use prism::events::ChangeSource;

struct Payment;
impl Payment {
    fn amount<S: ChangeSource<Update = u64> + Clone>(balance: S) -> FormItem {
        let valid = Validator::watch(balance, |_: &mut Context, balance: &Watched<S>, value: String| {
            let Some(available) = balance.latest() else {return FormValidState::Invalid};
            match value.parse::<u64>() {
                Ok(amount) if amount > 0 && amount <= *available => FormValidState::Valid,
                _ => FormValidState::Invalid,
            }
        });
        FormItem::number("Amount", NumberVariant::Number, valid)
    }
}
```

Sources and callbacks must be clonable because forms are clonable. Cloning a
validator follows the source's clone semantics and retains its last update;
updates themselves do not need to implement `Clone`. A player emits frame updates,
so a validator watching it runs as frames arrive. Pending notifications are
coalesced before validation, with bounded draining to keep the UI responsive.

## Numbers with changing values

Use a plain value for a fixed display: `Display::currency(12.50, "Available")` or
`Display::number(42.0, "Items")`. For a changing value, pass
`Number::watch(source, callback)`. The display updates through `OnChange` and only
rebuilds its digit slots when the formatted text changes.

```rust
use chk::{Display, Number};
use prism::events::ChangeSource;

struct Balance;
impl Balance {
    fn display<S: ChangeSource<Update = f64> + Clone>(source: S, initial: f64) -> Display {
        let number = Number::watch(source, move |_, latest| latest.copied().unwrap_or(initial));
        Display::currency(number, "Available")
    }
}
```

The callback receives `&S` and `Option<&S::Update>`. It reads the initial value
when the number is constructed and refreshes it when a display is built. Updates
are coalesced before recalculating, with bounded draining. For an Air instance,
the callback can read `instance.pending()`; for update-only sources, it can use
`latest`, which is `None` until the first notification. The last number remains
displayed after the source completes.

Sources and callbacks must be clonable. Cloning keeps the last received update
and follows the source's subscription semantics; updates need not be clonable.
Every dependency that can change the displayed number must notify the watched
source. Arbitrary getter closures are no longer polled each tick.

## Platform Support

* Linux
* Windows
* macOS
* Android
* iOS
* Web

## Examples

Example applications demonstrating CHK are included in the repository. 

Developers are encouraged to explore these examples to understand how CHK structures applications and generates interfaces automatically.

## Discord

https://discord.gg/53ERRpS4S4

Join the Discord server to ask questions, discuss development, or share projects.
