/*
 * Created on Thu Sep 23 2021
 *
 * This file is a part of Skytable
 * Skytable (formerly known as TerrabaseDB or Skybase) is a free and open-source
 * NoSQL database written by Sayan Nandan ("the Author") with the
 * vision to provide flexibility in data modelling without compromising
 * on performance, queryability or scalability.
 *
 * Copyright (c) 2021, Sayan Nandan <ohsayan@outlook.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 *
*/

use super::{BGSave, Configset, PortConfig, SnapshotConfig, SnapshotPref, SslOpts, DEFAULT_IPV4};
pub(super) use libsky::TResult;
use std::fs;

// server tests
// TCP
#[test]
fn server_tcp() {
    let mut cfgset = Configset::new_env();
    cfgset.server_tcp(
        Some("127.0.0.1"),
        "SKY_SERVER_HOST",
        Some("2004"),
        "SKY_SERVER_PORT",
    );
    assert_eq!(
        cfgset.cfg.ports,
        PortConfig::new_insecure_only(DEFAULT_IPV4, 2004)
    );
    assert!(cfgset.is_mutated());
    assert!(cfgset.is_okay());
}

#[test]
fn server_tcp_fail_host() {
    let mut cfgset = Configset::new_env();
    cfgset.server_tcp(
        Some("?127.0.0.1"),
        "SKY_SERVER_HOST",
        Some("2004"),
        "SKY_SERVER_PORT",
    );
    assert_eq!(
        cfgset.cfg.ports,
        PortConfig::new_insecure_only(DEFAULT_IPV4, 2004)
    );
    assert!(cfgset.is_mutated());
    assert!(!cfgset.is_okay());
}

#[test]
fn server_tcp_fail_port() {
    let mut cfgset = Configset::new_env();
    cfgset.server_tcp(
        Some("127.0.0.1"),
        "SKY_SERVER_HOST",
        Some("65537"),
        "SKY_SERVER_PORT",
    );
    assert_eq!(
        cfgset.cfg.ports,
        PortConfig::new_insecure_only(DEFAULT_IPV4, 2003)
    );
    assert!(cfgset.is_mutated());
    assert!(!cfgset.is_okay());
}

#[test]
fn server_tcp_fail_both() {
    let mut cfgset = Configset::new_env();
    cfgset.server_tcp(
        Some("?127.0.0.1"),
        "SKY_SERVER_HOST",
        Some("65537"),
        "SKY_SERVER_PORT",
    );
    assert_eq!(
        cfgset.cfg.ports,
        PortConfig::new_insecure_only(DEFAULT_IPV4, 2003)
    );
    assert!(cfgset.is_mutated());
    assert!(!cfgset.is_okay());
}

// noart
#[test]
fn server_noart_okay() {
    let mut cfgset = Configset::new_env();
    cfgset.server_noart(Some("true"), "SKY_SYSTEM_NOART");
    assert!(!cfgset.cfg.is_artful());
    assert!(cfgset.is_okay());
    assert!(cfgset.is_mutated());
}

#[test]
fn server_noart_fail() {
    let mut cfgset = Configset::new_env();
    cfgset.server_noart(Some("truee"), "SKY_SYSTEM_NOART");
    assert!(cfgset.cfg.is_artful());
    assert!(!cfgset.is_okay());
    assert!(cfgset.is_mutated());
}

#[test]
fn server_maxcon_okay() {
    let mut cfgset = Configset::new_env();
    cfgset.server_maxcon(Some("12345"), "SKY_SYSTEM_MAXCON");
    assert!(cfgset.is_mutated());
    assert!(cfgset.is_okay());
    assert_eq!(cfgset.cfg.maxcon, 12345);
}

#[test]
fn server_maxcon_fail() {
    let mut cfgset = Configset::new_env();
    cfgset.server_maxcon(Some("12345A"), "SKY_SYSTEM_MAXCON");
    assert!(cfgset.is_mutated());
    assert!(!cfgset.is_okay());
    assert_eq!(cfgset.cfg.maxcon, 50000);
}

// bgsave settings
#[test]
fn bgsave_okay() {
    let mut cfgset = Configset::new_env();
    cfgset.bgsave_settings(
        Some("true"),
        "SKY_BGSAVE_ENABLED",
        Some("128"),
        "SKY_BGSAVE_DURATION",
    );
    assert!(cfgset.is_mutated());
    assert!(cfgset.is_okay());
    assert_eq!(cfgset.cfg.bgsave, BGSave::Enabled(128));
}

#[test]
fn bgsave_fail() {
    let mut cfgset = Configset::new_env();
    cfgset.bgsave_settings(
        Some("truee"),
        "SKY_BGSAVE_ENABLED",
        Some("128"),
        "SKY_BGSAVE_DURATION",
    );
    assert!(cfgset.is_mutated());
    assert!(!cfgset.is_okay());
    assert_eq!(cfgset.cfg.bgsave, BGSave::Enabled(128));
}

// snapshot settings
#[test]
fn snapshot_okay() {
    let mut cfgset = Configset::new_env();
    cfgset.snapshot_settings(
        Some("3600"),
        "SKY_SNAPSHOT_EVERY",
        Some("0"),
        "SKY_SNAPSHOT_ATMOST",
        Some("false"),
        "SKY_SNAPSHOT_FAILSAFE",
    );
    assert!(cfgset.is_mutated());
    assert!(cfgset.is_okay());
    assert_eq!(
        cfgset.cfg.snapshot,
        SnapshotConfig::Enabled(SnapshotPref::new(3600, 0, false))
    );
}

#[test]
fn snapshot_fail() {
    let mut cfgset = Configset::new_env();
    cfgset.snapshot_settings(
        Some("3600"),
        "SKY_SNAPSHOT_EVERY",
        Some("0"),
        "SKY_SNAPSHOT_ATMOST",
        Some("falsee"),
        "SKY_SNAPSHOT_FAILSAFE",
    );
    assert!(cfgset.is_mutated());
    assert!(!cfgset.is_okay());
    assert_eq!(
        cfgset.cfg.snapshot,
        SnapshotConfig::Enabled(SnapshotPref::new(3600, 0, true))
    );
}

#[test]
fn snapshot_fail_with_missing_required_values() {
    let mut cfgset = Configset::new_env();
    cfgset.snapshot_settings(
        Some("3600"),
        "SKY_SNAPSHOT_EVERY",
        None,
        "SKY_SNAPSHOT_ATMOST",
        None,
        "SKY_SNAPSHOT_FAILSAFE",
    );
    assert!(cfgset.is_mutated());
    assert!(!cfgset.is_okay());
    assert_eq!(cfgset.cfg.snapshot, SnapshotConfig::Disabled);
}

// TLS settings
#[test]
fn tls_settings_okay() {
    let mut cfg = Configset::new_env();
    cfg.tls_settings(
        Some("key.pem"),
        "SKY_TLS_KEY",
        Some("cert.pem"),
        "SKY_TLS_CERT",
        Some("2005"),
        "SKY_TLS_PORT",
        Some("false"),
        "SKY_TLS_ONLY",
        None,
        "SKY_TLS_PASSIN",
    );
    assert!(cfg.is_mutated());
    assert!(cfg.is_okay());
    assert_eq!(cfg.cfg.ports, {
        let mut pf = PortConfig::default();
        pf.upgrade_to_tls(SslOpts::new(
            "key.pem".to_owned(),
            "cert.pem".to_owned(),
            2005,
            None,
        ));
        pf
    });
}

#[test]
fn tls_settings_fail() {
    let mut cfg = Configset::new_env();
    cfg.tls_settings(
        Some("key.pem"),
        "SKY_TLS_KEY",
        Some("cert.pem"),
        "SKY_TLS_CERT",
        Some("A2005"),
        "SKY_TLS_PORT",
        Some("false"),
        "SKY_TLS_ONLY",
        None,
        "SKY_TLS_PASSIN",
    );
    assert!(cfg.is_mutated());
    assert!(!cfg.is_okay());
    assert_eq!(cfg.cfg.ports, {
        let mut pf = PortConfig::default();
        pf.upgrade_to_tls(SslOpts::new(
            "key.pem".to_owned(),
            "cert.pem".to_owned(),
            2004,
            None,
        ));
        pf
    });
}

#[test]
fn tls_settings_fail_with_missing_required_values() {
    let mut cfg = Configset::new_env();
    cfg.tls_settings(
        Some("key.pem"),
        "SKY_TLS_KEY",
        None,
        "SKY_TLS_CERT",
        Some("2005"),
        "SKY_TLS_PORT",
        Some("false"),
        "SKY_TLS_ONLY",
        None,
        "SKY_TLS_PASSIN",
    );
    assert!(cfg.is_mutated());
    assert!(!cfg.is_okay());
    assert_eq!(cfg.cfg.ports, PortConfig::default());
}

/// Gets a `toml` file from `WORKSPACEROOT/examples/config-files`
fn get_toml_from_examples_dir(filename: &str) -> TResult<String> {
    use std::path;
    let curdir = path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspaceroot = curdir.ancestors().nth(1).unwrap();
    let mut fileloc = path::PathBuf::from(workspaceroot);
    fileloc.push("examples");
    fileloc.push("config-files");
    fileloc.push(filename);
    Ok(fs::read_to_string(fileloc)?)
}

mod cfg_file_tests {
    use super::get_toml_from_examples_dir;
    use crate::config::{
        cfgfile, ConfigurationSet, PortConfig, SnapshotConfig, SnapshotPref, SslOpts,
    };

    #[test]
    fn config_file_okay() {
        let file = get_toml_from_examples_dir("template.toml").unwrap();
        let toml = toml::from_str(&file).unwrap();
        let cfg_from_file = cfgfile::from_file(toml);
        assert!(cfg_from_file.is_mutated());
        assert!(cfg_from_file.is_okay());
        // expected
        let mut expected = ConfigurationSet::default();
        expected.snapshot = SnapshotConfig::Enabled(SnapshotPref::new(3600, 4, true));
        expected.ports = PortConfig::new_secure_only(
            crate::config::DEFAULT_IPV4,
            SslOpts::new(
                "/path/to/keyfile.pem".to_owned(),
                "/path/to/chain.pem".to_owned(),
                2004,
                Some("/path/to/cert/passphrase.txt".to_owned()),
            ),
        );
        // check
        assert_eq!(cfg_from_file.cfg, expected);
    }
}

mod try_from_config_source_impls {
    use crate::config::{cfgcli::Flag, cfgfile::Optional, TryFromConfigSource, DEFAULT_IPV4};
    use std::env::{set_var, var};
    use std::fmt::Debug;

    const EXPECT_TRUE: bool = true;
    const EXPECT_FALSE: bool = false;
    const MUTATED: bool = true;
    const NOT_MUTATED: bool = false;
    const IS_PRESENT: bool = true;
    const IS_ABSENT: bool = false;
    const MUTATION_FAILURE: bool = true;
    const NO_MUTATION_FAILURE: bool = false;

    fn _mut_base_test_expected<T: Default + PartialEq + Debug>(
        new: impl TryFromConfigSource<T>,
        expected: T,
        is_present: bool,
        mutate_failed: bool,
        has_mutated: bool,
    ) {
        let mut default = Default::default();
        let mut mutated = false;
        assert_eq!(new.is_present(), is_present);
        assert_eq!(new.mutate_failed(&mut default, &mut mutated), mutate_failed);
        assert_eq!(mutated, has_mutated);
        assert_eq!(default, expected);
    }

    fn _mut_base_test<T>(
        new: impl TryFromConfigSource<T>,
        mut default: T,
        is_present: bool,
        mutate_failed: bool,
        has_mutated: bool,
    ) {
        let mut mutated = false;
        dbg!(new.is_present(), is_present);
        assert_eq!(new.is_present(), is_present);
        assert_eq!(new.mutate_failed(&mut default, &mut mutated), mutate_failed);
        assert_eq!(mutated, has_mutated);
    }

    fn mut_test_pass<T>(new: impl TryFromConfigSource<T>, default: T) {
        _mut_base_test(new, default, IS_PRESENT, NO_MUTATION_FAILURE, MUTATED)
    }

    fn mut_test_fail<T>(new: impl TryFromConfigSource<T>, default: T) {
        _mut_base_test(new, default, IS_PRESENT, MUTATION_FAILURE, MUTATED)
    }

    mod env_var {
        use super::*;

        // test for Result<String, VarError>
        #[test]
        fn env_okay_ipv4() {
            set_var("TEST_SKY_SYSTEM_HOST", "127.0.0.1");
            mut_test_pass(var("TEST_SKY_SYSTEM_HOST"), DEFAULT_IPV4);
        }

        #[test]
        fn env_fail_ipv4() {
            set_var("TEST_SKY_SYSTEM_HOST2", "127.0.0.1A");
            mut_test_fail(var("TEST_SKY_SYSTEM_HOST2"), DEFAULT_IPV4);
        }
    }

    mod option_str {
        use super::*;

        // test for Option<&str> (as in CLI)
        #[test]
        fn option_str_okay_ipv4() {
            let ip = Some("127.0.0.1");
            mut_test_pass(ip, DEFAULT_IPV4);
        }

        #[test]
        fn option_str_fail_ipv4() {
            let ip = Some("127.0.0.1A");
            mut_test_fail(ip, DEFAULT_IPV4);
        }

        #[test]
        fn option_str_nomut() {
            let ip = None;
            _mut_base_test(
                ip,
                DEFAULT_IPV4,
                IS_ABSENT,
                NO_MUTATION_FAILURE,
                NOT_MUTATED,
            );
        }
    }

    mod cfgcli_flag {
        use super::*;

        #[test]
        fn flag_true_if_set_okay_set() {
            // this is true if flag is present
            let flag = Flag::<true>::new(true);
            // we expect true
            _mut_base_test_expected(flag, EXPECT_TRUE, IS_PRESENT, NO_MUTATION_FAILURE, MUTATED);
        }

        #[test]
        fn flag_true_if_set_okay_unset() {
            // this is true if flag is present, but the flag here is not present
            let flag = Flag::<true>::new(false);
            // we expect no mutation because the flag was not set
            _mut_base_test(
                flag,
                EXPECT_FALSE,
                IS_ABSENT,
                NO_MUTATION_FAILURE,
                NOT_MUTATED,
            );
        }

        #[test]
        fn flag_false_if_set_okay_set() {
            // this is false if flag is present
            let flag = Flag::<false>::new(true);
            // expect mutation to have happened
            _mut_base_test_expected(flag, EXPECT_FALSE, IS_PRESENT, NO_MUTATION_FAILURE, MUTATED);
        }

        #[test]
        fn flag_false_if_set_okay_unset() {
            // this is false if flag is present, but the flag is absent
            let flag = Flag::<true>::new(false);
            // expect no mutation
            _mut_base_test(
                flag,
                EXPECT_FALSE,
                IS_ABSENT,
                NO_MUTATION_FAILURE,
                NOT_MUTATED,
            );
        }
    }

    mod optional {
        use super::*;

        // test for cfg file scenario
        #[test]
        fn optional_okay_ipv4() {
            let ip = Optional::some(DEFAULT_IPV4);
            mut_test_pass(ip, DEFAULT_IPV4);
        }

        #[test]
        fn optional_okay_ipv4_none() {
            let ip = Optional::from(None);
            _mut_base_test(
                ip,
                DEFAULT_IPV4,
                IS_ABSENT,
                NO_MUTATION_FAILURE,
                NOT_MUTATED,
            );
        }
    }

    mod cfgfile_nonull {
        use super::*;
        use crate::config::cfgfile::NonNull;

        #[test]
        fn nonnull_okay() {
            let port = NonNull::from(2100);
            _mut_base_test_expected(port, 2100, IS_PRESENT, NO_MUTATION_FAILURE, MUTATED);
        }
    }

    mod optstring {
        use super::*;
        use crate::config::OptString;

        #[test]
        fn optstring_okay() {
            let pass = OptString::from(Some("tlspass.txt".to_owned()));
            _mut_base_test_expected(
                pass,
                OptString::from(Some("tlspass.txt".to_owned())),
                IS_PRESENT,
                NO_MUTATION_FAILURE,
                MUTATED,
            );
        }

        #[test]
        fn optstring_null_okay() {
            let pass = OptString::from(None);
            _mut_base_test_expected(
                pass,
                OptString::new_null(),
                IS_PRESENT,
                NO_MUTATION_FAILURE,
                NOT_MUTATED,
            );
        }
    }
}
