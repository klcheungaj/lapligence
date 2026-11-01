//! The LTO request and final-link arguments that build.rs derives for the
//! native Slang frontend (build_support/native_lto.rs).

#[allow(clippy::module_inception)]
#[path = "../build_support/native_lto.rs"]
mod native_lto;

use native_lto::{is_enabled, link_args, request, Request};

#[test]
fn default_enables_lto_in_release_profiles_only() {
    assert_eq!(request("", "release"), Ok(Request::Auto));
    assert_eq!(request("auto", "release"), Ok(Request::Auto));
    assert_eq!(request(" AUTO ", "release"), Ok(Request::Auto));
    assert_eq!(request("", "debug"), Ok(Request::Off));
    assert_eq!(request("auto", "debug"), Ok(Request::Off));
}

#[test]
fn explicit_settings_override_the_profile() {
    for setting in ["1", "on", "ON", "true"] {
        assert_eq!(request(setting, "debug"), Ok(Request::On), "{setting}");
        assert_eq!(request(setting, "release"), Ok(Request::On), "{setting}");
    }
    for setting in ["0", "off", "Off", "false"] {
        assert_eq!(request(setting, "release"), Ok(Request::Off), "{setting}");
    }
    assert!(request("thin", "release").is_err());
    assert!(request("2", "debug").is_err());
}

#[test]
fn lto_archives_live_in_their_own_build_directory() {
    assert_eq!(Request::Off.build_dir_suffix(), "");
    assert_eq!(Request::Auto.build_dir_suffix(), "-lto");
    assert_eq!(Request::On.build_dir_suffix(), "-lto");
    assert_eq!(Request::Off.cmake_value(), "OFF");
    assert_eq!(Request::Auto.cmake_value(), "AUTO");
    assert_eq!(Request::On.cmake_value(), "ON");
}

#[test]
fn link_arguments_follow_the_recorded_compiler_family() {
    assert_eq!(link_args("GNU\n", "linux", "musl"), ["-flto=auto"]);
    assert_eq!(link_args("MSVC\n", "windows", "msvc"), ["/LTCG"]);
    // The linker reads LLVM bitcode itself; a GCC `cc` driver would reject
    // `-flto=thin`.
    assert!(link_args("Clang\n", "linux", "gnu").is_empty());
    assert!(link_args("AppleClang\n", "macos", "").is_empty());
    assert!(link_args("off\n", "linux", "gnu").is_empty());
    assert!(is_enabled("GNU\n"));
    assert!(is_enabled("AppleClang"));
    assert!(!is_enabled("off\n"));
    assert!(!is_enabled(""));
}

#[test]
fn gcc_lto_on_linux_gnu_avoids_rustc_bundled_lld() {
    // rustc's default lld on x86_64 Linux GNU cannot run GCC's plugin.
    assert_eq!(
        link_args("GNU", "linux", "gnu"),
        ["-flto=auto", "-fuse-ld=bfd"]
    );
    assert_eq!(link_args("GNU", "macos", ""), ["-flto=auto"]);
}
