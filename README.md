# CHK Overview

## Design-system selection

Each app chooses a type implementing `chk::DesignSystem`:

```rust
impl chk::App for MyApp {
    type DesignSystem = chk::PelicanUi;
    type Invitation = MyInvitation;
    // name, new, roots, and the app's other configuration...
}
```

The optional feature makes the bundled Pelican adapter available:

```toml
chk = { version = "2.0.0", features = ["pelican_ui"] }
```

For an external design system, omit that feature and choose the external type:

```toml
chk = { version = "2.0.0", default-features = false }
my_design_system = "1.0"
```

```rust
impl chk::App for MyApp {
    type DesignSystem = my_design_system::MyDesignSystem;
    type Invitation = MyInvitation;
    // ...
}
```

CHK compiles without Pelican. `App::DesignSystem` has no default, so every app
must provide its implementation. The selected backend travels with the app's
`Theme`; no global registration or CHK source changes are needed for another
adapter. Explicit theme creation uses `ChkTheme::build::<MyDesignSystem>(assets)`.

`DesignSystem` maps prepared descriptions to native constructors. For example,
its `list_item` method reads CHK's public `ListItem` fields and constructs the
native row. The Pelican mappings live in `src/pelican_ui.rs`. Controls implement
ordinary Rust traits such as `chk::Slider::value`, `TextField`, and `InputControl`.
`Control::boxed` preserves their state interface inside a drawable tree.

`src/pelican_ui.rs` is the complete Pelican adapter template: widget construction,
callback translation, control/surface implementations, and the legacy
`ChkTheme::to_pelican` convenience method all live there. Other adapters can copy
its structure and supply their own widgets. Adapters in other crates use `chk::`
for CHK types and wrap foreign widgets in owned newtypes to implement CHK's traits
where Rust's orphan rules require it. Outside that file, Pelican implementation
references appear only in the optional dependency/feature and module/export declarations.

CHK owns page assembly, labels, validation, filtering, navigation, submission,
read receipts, typing, and media lifecycles. Page composition stays alongside
its struct. Shared content, events, and serialized models live in PTSD, so
adapters reuse the same form behavior and stored data. Native widgets own their
layout and pointer handling.

CHK functions are public associated methods on their owning types; trait implementations
use Rust's required visibility syntax. Inline one-use glue instead of adding private
helpers. Constructors initialize their own types directly. Do not add tests to CHK.
Follow Ramp's `STYLE_GUIDE.md`.
CHK has no direct `futures` or `async-channel` dependency;
its asynchronous state uses standard-library futures and wakers.

## Frontend development setup

Cargo retrieves Pelican UI 3.0.0 and PTSD 0.2.0 from pinned Git revisions, with backend
dependencies from crates.io. Keep the committed lockfile; sibling checkouts are optional.
Use local Cargo overrides when editing multiple frontend crates together, and keep those
overrides out of release manifests. Follow the [Orange setup instructions](https://github.com/ramp-stack/orange#frontend-setup)
for Cargo Piston, the shared team identity, and build troubleshooting.

CHK is a Rust application-building layer designed to sit on top of UI systems built with **Prism**. Rather than requiring developers to construct user interfaces manually, CHK allows builders to describe the **data they want to collect or display**, and automatically generates the corresponding application interface.

Instead of choosing specific UI components such as text inputs, radio selectors, or tables, developers simply define the information a user should interact with. CHK then determines how that information should be presented within the underlying UI system.

CHK currently supplies a **Pelican UI** adapter; its public `DesignSystem` trait accepts adapters from external crates.

## Page items and standard screens

`Item::Input(Input::...)` and `Item::Display(Display::...)` explicitly distinguish
editable controls from presentation. `PageType::content` accepts an ordered list
of these items. The typed `PageType::input` and `PageType::display` constructors
remain conveniences; both now produce the same `Content` variant.

```rust
let page = PageType::content("Details", vec![
    Item::Display(Display::text("Choose a contact to continue.")),
    Item::Input(Input::selection(selector)),
], None, Bumper::Default, Offset::Start);
```

Use `FormItem` for fields requiring validation and submission. `PageType::edit`
accepts those fields directly with optional display content and a header action;
there is no separate `EditAndDisplay` variant. `Form::page` uses the same editor.
`Form::flow` still provides one field per step, with optional `Review` and `Success`.

Standard screens are associated functions on `Root`, `Page`, `PageType`, and `Form`. Chat applications can construct
their navigation directly, including the user's avatar on the Profile tab.
The Profile tab watches the saved avatar, including synced changes, and refreshes
when shown if changes arrived while navigation was hidden:

```rust
vec![
    RootInfo::messages(ctx, theme, invitations),
    RootInfo::contacts(ctx, theme, invitations),
    RootInfo::profile(ctx, theme, invitations),
]
```

`RootInfo::messages` starts CHK's membership-aware notification subscription service.
`Root::{messages, contacts, profile}` supplies the underlying root pages.
`Form::{new_message, new_contact, select_contact}` supplies the individual flows,
with a `ContactInvitations` argument for contact link parsing.
The message list watches rooms, profiles, and per-reader read state. Contacts and
messages retain their existing navigation, sorting, unread badges, and previews.
All contact identity labels use **AirName**. Apps provide their invitation URL,
parser, and receive action through `App::invitations`; no Orange URLs or branding
are embedded in these screens. `ContactInvitations::default()` supports raw AirNames
without requiring an invitation configuration. `select_contact` also supports
an exclusion list, used by the group-member picker.

API migration from the previous descriptions:

- `PageType::{Display, Input, Both}` → `PageType::Content`.
- `PageType::EditAndDisplay` → `PageType::Edit`.
- `PageType::edit` now takes `FormItem`s and an optional header, with validators
  derived from the fields rather than a separate validation list.
- `InputAction` tuples → `ActionItem`; `FormAction::Custom` takes an `ActionItem`.
- `TableItem` and `EnumItem` → `ValueItem`.
- Unused form-storage callbacks and unused closure traits have been removed.
- Headers, root bumper actions, and `ListItem::action` accept `Action`. Use
  `Action::deferred_flow` when a flow needs current context at activation time.
- Chat actions live on `Action::{message, block, unblock}`.
- CHK's `Selector` describes choices and scoped actions independently of the UI.
  The selected design-system adapter creates its selection control;
  `Selector::select` returns a CHK callback.
- Form submit, review, and success callbacks receive `FormValues`, replacing
  positional state vectors.

## Named fields and saving

Give each form field a stable key with `.named("username")`. Without an explicit
key, the field's title is used. Keys must be nonempty and unique across the form.
Read values using typed accessors such as `values.text("username")`,
`values.avatar("avatar")`, or `values.selection("members")`; `values.get(key)`
returns the underlying `State` when needed. Reordering fields or translating
labels does not change callbacks when fields have explicit keys.

Return `FormComplete::None` on success to continue normally, or
`FormComplete::Error(message)` to show a failure and retain the draft. Return
`FormComplete::pending(future)` for asynchronous work; the future returns another
`FormComplete`. CHK disables submission and blocks navigation while it is pending.
Return a completion action instead of navigating from inside the submit callback,
so CHK can finish save bookkeeping before navigation.

Edit pages enable Save only for valid changes. Successful saves update the
baseline to the submitted values; failed saves keep the draft dirty. Edits made
while a save is pending remain unsaved. Leaving an active dirty editor offers
Save and leave, Discard changes, or Keep editing. Pushed pickers and dialogs retain
the editor underneath them. `App::build` installs the form host automatically;
custom interfaces should wrap their interface in `FormHost`.

## Group conversations

The group info screen provides name/photo editing, contact selection (including a new
AirName or QR code), and a confirmation before leaving. Any current member can
edit the group or invite someone. New members receive the existing conversation history;
available video attachments are shared separately. Leaving hides the room in Orange,
disables notifications and sending, and preserves its group identity even with fewer
than three members. Another current member can invite a departed member again.

Adding or leaving writes a typed, read-only membership notice in the same AIR update.
The conversation renders it as plain centered text, with no avatar, bubble, sender
label, or tap action. List previews use the same notice without a sender prefix.
Group names and photos are shared room data; personal contact edits do not change them.

Group actions are appended to the existing action enum so old signed messages still
verify. Existing room caches load without a reset. Updated group snapshots use an empty
leading members vector as a version marker; ordinary DM snapshots keep their old bytes.
All participants should update to use group controls consistently. Older app versions
do not understand the new actions or upgraded group caches.

Leaving is an application membership change, not cryptographic revocation: AIR currently
exposes sharing but no unshare/key rotation API. Former members retain existing channel
credentials. Enforcing read-access revocation requires backend support.

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

## Searchable selection

Use one `Selector` for contacts or application-owned records. Keys are strings; the
selector does not create identities, load profiles, or access AIR. Each handle has
its own event scope, and clones refer to that same input.

```rust
let selector = Selector::new("passwords", vec![
    (ListItem::plain("Amazon", "family@example.test", None, None), "login-123".into()),
]).with_limit(32).with_chip_subtitles(true);
let field = FormItem::selection("Choose passwords", selector.clone()).named("passwords");
// Submit reads values.selection("passwords").
```

The standard form supplies searchable rows, removable selected chips, and Continue
validation requiring at least one selection. Search filters visible titles and
subtitles in place. Limits count unique keys; empty and duplicate initial keys are
ignored. Selected chips show titles by default; opt into subtitles when they help
distinguish records. Applications must still validate returned keys against their
current data and permissions before performing an action.

Attach `FormAction`s with `selector.with_actions(...)`. When a nested flow creates a
record, return `FormComplete::Return(selector.select(item, key).get())` to add and
select its row in the original selector. Repeating this action updates the row and
keeps it selected. Create a fresh `Selector` for each independent input.

Legacy `FormItem::search` and `Input::search` use this same control and continue to
return `State::Search(Vec<Name>)`, available through `values.search(key)`.
Use the scoped `Selector::select` action to add a choice from a nested flow.

## Application events

`App::build` constructs the interface without retaining the application in an event callback.
Handle application events with `#[on_event] impl OnEvent<E>` on the component that owns the
behavior. Pages handle local actions; an application-owned root wrapping the built interface
can handle actions shared by multiple pages. That root receives events before the interface, and
returns the events that should continue to the interface.

## Refreshing pages

`Page::watching` subscribes to Air changes through Prism's `OnChange`. `ContractWatch::new`
watches one instance; `ListWatch::default` watches both new instances and changes to existing
members. Tuples combine sources, for example a chat room and the block list.

`Listener::new` receives navigation information before building its initial page, so
construction builds once. Subsequent notifications rebuild the page with the same
navigation and preserve message drafts. Notifications coalesce while a page is hidden;
reactivation reloads its snapshot before consuming a potentially lagged queue. Idle polls do not clone or compare contract
snapshots, and watched contracts no longer need `PartialEq`.

`Listener` remains the owner of the built page and its builder. Custom `PageBuilder`
implementations provide a cancelable `next()` future returning `Some(())` when a rebuild is
needed, or `None` when there are no further notifications. A cloned watched page refreshes
once on activation to reconcile changes that preceded its new subscriptions.

## Validators with changing dependencies

Ordinary closures remain valid form validators. When validity also depends on a
changing value, pass `Validator::watch(source, listen, validate)` instead. `listen` is an async
callback that borrows the source and returns its next update. The form checks
initially, after input edits, and when the source wakes it; no `TickEvent`, manual
refresh, or page rebuild is needed.

The validation callback receives `&Watched<S, U>`. It dereferences to the source, so Air instance
methods such as `pending()` and player methods such as `frame()` remain available.
For a source that delivers values only through updates, `latest()` returns the
last update, or `None` before the first update. The last update remains available
after the source completes and when the user edits an input again.

For example, a source delivering available amounts as `u64` can validate a number:

```rust
use chk::{Context, FormItem, FormValidState, NumberVariant, Validator, Watched};

struct Payment;
impl Payment {
    fn amount<S: Clone + 'static>(balance: S, listen: impl AsyncFnMut(&mut S) -> u64 + Clone + 'static) -> FormItem {
        let valid = Validator::watch(
            balance, listen, |_: &mut Context, balance: &Watched<S, u64>, value: String| {
                let Some(available) = balance.latest() else {return FormValidState::Invalid};
                match value.parse::<u64>() {
                    Ok(amount) if amount > 0 && amount <= *available => FormValidState::Valid,
                    _ => FormValidState::Invalid,
                }
            },
        );
        FormItem::number("Amount", NumberVariant::Number, valid)
    }
}
```

For an Air instance, the listener can be `async |instance| instance.next().await`.
Sources and callbacks must be clonable because forms are clonable. Cloning a
validator follows the source's clone semantics and retains its last update;
updates themselves do not need to implement `Clone`. A player emits frame updates,
so a validator watching it runs as frames arrive. Listener futures must be safe to
cancel and recreate. When an optional source is absent or a one-shot source has finished,
the listener should await `std::future::pending()` until the component changes its source.

## Numbers with changing values

Use a plain value for a fixed display: `Display::currency(12.50, "Available")` or
`Display::number(42.0, "Items")`. For a changing value, pass
`Number::watch(source, listen, value)`. The display updates through `OnChange` and only
rebuilds its digit slots when the formatted text changes.

```rust
use chk::{Display, Number};

struct Balance;
impl Balance {
    fn display<S: Clone + 'static>(source: S, listen: impl AsyncFnMut(&mut S) -> f64 + Clone + 'static, initial: f64) -> Display {
        let number = Number::watch(source, listen, move |_, latest| latest.copied().unwrap_or(initial));
        Display::currency(number, "Available")
    }
}
```

The value callback receives `&S` and `Option<&U>`, where `U` is the listener's output. It reads
the initial value when the number is constructed and refreshes it when a display is built.
For an Air instance, the callback can read `instance.pending()`; for update-only sources, it can use
`latest`, which is `None` until the first notification. The last number remains
displayed after the source completes.

Sources and callbacks must be clonable. Cloning keeps the last received update
and follows the source's subscription semantics; updates need not be clonable.
Every dependency that can change the displayed number must notify the watched
source. Arbitrary getter closures are no longer polled each tick.

## Unread messages

`ListItem::with_badge(true)` displays a small brand-colored dot beside the title.
Orange watches rooms and `RoomReadState` instances to refresh these badges when messages
arrive or are read. Own messages and messages hidden from the reader do not count as unread.

`RoomReadState` stores a boolean per message ID in a private, per-reader AIR instance,
keyed by the room's instance ID. Missing flags mean unread. This preserves the existing
binary format of shared messages and delete actions, and keeps group members' read state
independent. Read-state instances are not shared and do not enable notifications.

The active `MessagesPage` acknowledges its displayed conversation snapshot. Constructing a
page does not mark it read, and background or hidden pages do not acknowledge messages.
Refreshing an open conversation acknowledges newly displayed incoming messages; reopening
the app retains saved flags. Existing history has no flags until first opened.

## Typing indicators

Messaging pages publish throttled activity to a separate `TypingState` AIR contract,
shared only with the room's other unblocked members. Activity contains a boolean and
timestamp, never draft text, and notifications stay disabled. It does not change chat
history, list previews, or unread badges. These are persisted AIR updates with UI expiry,
not an ephemeral transport.

The first edit publishes immediately; subsequent edits publish at most once every two
seconds. Three seconds without editing, sending, losing focus, backgrounding, or leaving
the conversation stops activity. Remote indicators expire after six seconds if a stop
update is lost or a client disconnects. Old queued activity does not revive on reconnect.

Pelican's `TypingIndicator` shows an animated three-dot received bubble. DMs have no label;
groups show a separate bubble and `Name is typing...` below it for each active sender.
Only the typing portion updates as activity changes, preserving the composer draft.

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
