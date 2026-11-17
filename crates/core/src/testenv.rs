//! The tests' switches: environment variables that point Harness Manager at a mock server, a fake tool or a
//! recorded terminal. They are read only through here, so an installed copy working on the real data never
//! follows one that was left in (or put into) its environment.

/// The switch `name`, when switches are honoured here: in a debug build, or beside `HRM_HOME` (the tests always
/// set it, also when they drive the installed release build).
pub fn var(name: &str) -> Option<String> {
    if !honoured(cfg!(debug_assertions), std::env::var_os("HRM_HOME").is_some()) { return None; }
    std::env::var(name).ok()
}

fn honoured(debug_build: bool, data_folder_redirected: bool) -> bool { debug_build || data_folder_redirected }

#[cfg(test)]
mod tests {
    #[test]
    fn switches_are_honoured_only_in_a_debug_build_or_beside_hrm_home() {
        assert!(!super::honoured(false, false), "a release build on the real data folder ignores them");
        assert!(super::honoured(false, true), "the installed build under test (HRM_HOME set) follows them");
        assert!(super::honoured(true, false), "a debug build on real data follows them");
        // in this build: a set switch is read exactly when the rule says so, an unset one never
        std::env::set_var("HRM_TESTENV_PROBE", "x");
        let on = super::honoured(cfg!(debug_assertions), std::env::var_os("HRM_HOME").is_some());
        assert_eq!(super::var("HRM_TESTENV_PROBE"), on.then(|| "x".to_string()));
        assert_eq!(super::var("HRM_TESTENV_UNSET"), None);
    }
}
