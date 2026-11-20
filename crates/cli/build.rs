// hrm.exe carries Windows version information (product, company, version), like the app's exe next to it. Without
// it Defender's cloud model reported the 1.11.3 hrm.exe as Trojan:Win32/Bearfoos.A!ml on 19 Nov 2026: an unsigned
// file with no properties that reads Credential Manager and starts processes. The app's exe has them and was not
// reported.
fn main() {
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        tauri_winres::WindowsResource::new()
            .set("ProductName", "Harness Manager")
            .set("FileDescription", "Harness Manager command line (hrm)")
            .set("CompanyName", "abushanab")
            .compile()
            .expect("version information for hrm.exe");
    }
}
