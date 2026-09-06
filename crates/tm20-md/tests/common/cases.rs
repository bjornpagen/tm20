// Explicit registration keeps fixture discovery out of downstream builds.
// Inventory tests fail if a fixture is added or removed without registration.
macro_rules! cases {
    ($module:ident, $runner:ident; $($name:ident => $stem:literal,)*) => {
        mod $module {
            pub const STEMS: &[&str] = &[$($stem,)*];
            $(#[test] fn $name() { super::$runner($stem); })*
        }
    };
}
