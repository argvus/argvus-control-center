//! GPG keys listed and changed through `argvus gpg` (the GPG keys page).
//!
//! The launcher owns every write: it creates keys, exports public keys, imports
//! them and writes the pinentry line of `gpg-agent.conf`. This module only calls
//! its CLI and parses the TSV it prints. Passphrases never pass through here:
//! a key with a passphrase is generated while the pinentry window asks for it,
//! so the secret is never a process argument.
use argvus_control_center_core::process::command;

/// Key algorithm offered by the generate form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyAlgorithm {
  #[default]
  Ed25519,
  Rsa4096,
}

impl KeyAlgorithm {
  /// The value `argvus gpg generate --algo` accepts.
  pub fn as_arg(self) -> &'static str {
    match self {
      Self::Ed25519 => "ed25519",
      Self::Rsa4096 => "rsa4096",
    }
  }

  /// The other algorithm; the form's algorithm row cycles between the two.
  pub fn toggled(self) -> Self {
    match self {
      Self::Ed25519 => Self::Rsa4096,
      Self::Rsa4096 => Self::Ed25519,
    }
  }
}

/// Values of the generate form. The passphrase flag is on by default, so the
/// pinentry asks for it; turning it off makes generation unprotected, which the
/// page confirms first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpgForm {
  pub identity: String,
  pub algorithm: KeyAlgorithm,
  pub expire: String,
  pub passphrase: bool,
}

impl Default for GpgForm {
  fn default() -> Self {
    Self {
      identity: String::new(),
      algorithm: KeyAlgorithm::default(),
      expire: "2y".into(),
      passphrase: true,
    }
  }
}

/// One primary key of the keyring, as `argvus gpg list` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpgKeyEntry {
  pub fingerprint: String,
  pub key_id: String,
  pub key_type: String,
  /// A date such as `2028-10-12`, or `never`.
  pub expires: String,
  /// Whether the secret key is in the keyring. Only public keys can be
  /// exported from the page, so this is informative.
  pub secret: bool,
  pub validity: String,
  pub user_id: String,
}

/// Where the pinentry setting comes from, as `argvus gpg status` reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PinentryOrigin {
  /// Written by ARGVUS, between its markers in `gpg-agent.conf`.
  Argvus,
  /// Written by the user, outside the ARGVUS block.
  User,
  /// Not set; gpg uses its built-in default.
  #[default]
  Default,
}

/// The pinentry program gpg-agent uses, if one is set.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pinentry {
  pub program: Option<String>,
  pub origin: PinentryOrigin,
}

/// The keyring and the pinentry. `available` is false when `argvus gpg` is not
/// installed, so the page can say so instead of showing an empty list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GpgKeys {
  pub available: bool,
  pub entries: Vec<GpgKeyEntry>,
  pub pinentry: Pinentry,
}

/// The pinentry choices the page offers, as `(choice, program)` pairs. The
/// choice is the word `argvus gpg set-pinentry` takes; the program is the
/// binary it writes into `gpg-agent.conf`.
pub const PINENTRY_CHOICES: [(&str, &str); 3] = [
  ("qt", "pinentry-qt"),
  ("gtk", "pinentry-gtk-2"),
  ("curses", "pinentry-curses"),
];

/// Loads the keyring and the pinentry, after checking that the launcher
/// command is installed.
pub fn load() -> GpgKeys {
  let entries = match command("argvus").args(["gpg", "list"]).output() {
    Ok(output) if output.status.success() => parse_list(&String::from_utf8_lossy(&output.stdout)),
    _ => return GpgKeys::default(),
  };
  GpgKeys {
    available: true,
    entries,
    pinentry: load_pinentry(),
  }
}

fn load_pinentry() -> Pinentry {
  match command("argvus").args(["gpg", "status"]).output() {
    Ok(output) if output.status.success() => parse_status(&String::from_utf8_lossy(&output.stdout)),
    _ => Pinentry::default(),
  }
}

/// Parses the TSV printed by `argvus gpg list`:
/// `fingerprint, key id, type, expires, secret, validity, user id`.
/// Lines with another field count are skipped, so a launcher that prints extra
/// columns in the future cannot break the page.
pub fn parse_list(listing: &str) -> Vec<GpgKeyEntry> {
  listing.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<GpgKeyEntry> {
  let fields: Vec<&str> = line.split('\t').collect();
  let [
    fingerprint,
    key_id,
    key_type,
    expires,
    secret,
    validity,
    user_id,
  ] = fields.as_slice()
  else {
    return None;
  };
  if fingerprint.is_empty() {
    return None;
  }
  Some(GpgKeyEntry {
    fingerprint: (*fingerprint).to_owned(),
    key_id: (*key_id).to_owned(),
    key_type: (*key_type).to_owned(),
    expires: (*expires).to_owned(),
    secret: *secret == "yes",
    validity: (*validity).to_owned(),
    user_id: (*user_id).to_owned(),
  })
}

/// Parses the `pinentry` line printed by `argvus gpg status`:
/// `pinentry, program or none, argvus|user|default`. Any other shape keeps the
/// default, so the page never claims a pinentry it cannot read.
pub fn parse_status(status: &str) -> Pinentry {
  for line in status.lines() {
    let fields: Vec<&str> = line.split('\t').collect();
    let ["pinentry", program, origin] = fields.as_slice() else {
      continue;
    };
    let origin = match *origin {
      "argvus" => PinentryOrigin::Argvus,
      "user" => PinentryOrigin::User,
      _ => PinentryOrigin::Default,
    };
    let program = (*program != "none").then(|| (*program).to_owned());
    return Pinentry { program, origin };
  }
  Pinentry::default()
}

/// The choice whose program is the one in use, for the page's marker.
pub fn pinentry_choice(pinentry: &Pinentry) -> Option<&'static str> {
  let path = pinentry.program.as_deref()?;
  let program = path.rsplit('/').next().unwrap_or(path);
  PINENTRY_CHOICES
    .iter()
    .find(|(_, name)| *name == program)
    .map(|(choice, _)| *choice)
}

/// Replaces a leading `~` with the home directory, since the launcher gets the
/// path as an argument and no shell expands it.
pub fn expand_home(path: &str, home: &str) -> String {
  match path.strip_prefix('~') {
    Some("") => home.to_owned(),
    Some(rest) if rest.starts_with('/') => format!("{home}{rest}"),
    _ => path.to_owned(),
  }
}

/// Generates a key from the form. The passphrase, when set, is typed in the
/// pinentry window that gpg-agent opens, so this call waits for that window.
/// Without a passphrase the launcher requires `--yes`, which the page asks for
/// before it calls this.
pub fn generate(form: &GpgForm) -> Result<(), String> {
  run(&generate_args(form))
}

/// The arguments of `argvus gpg generate` for a form. Kept apart from the
/// process call so the rules are testable without running anything.
pub fn generate_args(form: &GpgForm) -> Vec<&str> {
  let mut args = vec![
    "generate",
    form.identity.trim(),
    "--algo",
    form.algorithm.as_arg(),
    "--expire",
    form.expire.trim(),
  ];
  if !form.passphrase {
    args.extend(["--no-passphrase", "--yes"]);
  }
  args
}

/// Writes the public key of one fingerprint to a file. The launcher refuses to
/// replace an existing file, so the page never overwrites one.
pub fn export_public(fingerprint: &str, path: &str) -> Result<(), String> {
  run(&["export-public", fingerprint, path])
}

/// Imports public keys from a file. The launcher refuses files that hold a
/// private key.
pub fn import(path: &str) -> Result<(), String> {
  run(&["import", path])
}

/// Writes the pinentry choice (`qt`, `gtk` or `curses`) to `gpg-agent.conf`.
pub fn set_pinentry(choice: &str) -> Result<(), String> {
  if !PINENTRY_CHOICES.iter().any(|(name, _)| *name == choice) {
    return Err(format!("unknown pinentry: {choice}"));
  }
  run(&["set-pinentry", choice, "--yes"])
}

fn run(args: &[&str]) -> Result<(), String> {
  let output = command("argvus")
    .arg("gpg")
    .args(args)
    .output()
    .map_err(|error| format!("failed to run argvus-gpg: {error}"))?;
  if output.status.success() {
    return Ok(());
  }
  let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
  Err(if stderr.is_empty() {
    "argvus-gpg failed".into()
  } else {
    stderr
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  const FP_ALICE: &str = "AAAA111122223333AAAA111122223333AAAAAAAA";
  const FP_BOB: &str = "CCCCCCCCCCCCCCCCDDDDDDDDDDDDDDDDEEEEEEEE";

  #[test]
  fn parses_each_listed_primary_key() {
    let listing = format!(
      "{FP_ALICE}\tAAAA111122223333\ted25519\tnever\tyes\tu\tAlice <alice@example.com>\n\
       {FP_BOB}\tCCCCCCCCCCCCCCCC\trsa\t2030-03-17\tno\te\tBob: Test <bob@example.com>\n"
    );
    let entries = parse_list(&listing);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].fingerprint, FP_ALICE);
    assert_eq!(entries[0].key_type, "ed25519");
    assert_eq!(entries[0].expires, "never");
    assert!(entries[0].secret);
    assert_eq!(entries[1].expires, "2030-03-17");
    assert!(!entries[1].secret);
    assert_eq!(entries[1].validity, "e");
    assert_eq!(entries[1].user_id, "Bob: Test <bob@example.com>");
  }

  #[test]
  fn skips_lines_with_another_column_count() {
    let entries = parse_list("AAAA\tAAAA\ted25519\n\n");
    assert!(entries.is_empty());
  }

  #[test]
  fn skips_lines_without_a_fingerprint() {
    let entries = parse_list("\tAAAA\ted25519\tnever\tyes\tu\tAlice\n");
    assert!(entries.is_empty());
  }

  #[test]
  fn reads_the_pinentry_line_and_its_origin() {
    let managed = parse_status("gnupg\tyes\npinentry\t/usr/bin/pinentry-qt\targvus\n");
    assert_eq!(managed.program.as_deref(), Some("/usr/bin/pinentry-qt"));
    assert_eq!(managed.origin, PinentryOrigin::Argvus);

    let user = parse_status("pinentry\t/usr/bin/pinentry-gtk-2\tuser\n");
    assert_eq!(user.origin, PinentryOrigin::User);

    let none = parse_status("pinentry\tnone\tdefault\n");
    assert_eq!(none, Pinentry::default());
  }

  #[test]
  fn an_unreadable_status_keeps_the_default_pinentry() {
    assert_eq!(parse_status("garbage\n"), Pinentry::default());
  }

  #[test]
  fn maps_the_pinentry_program_to_its_choice() {
    let qt = Pinentry {
      program: Some("/usr/bin/pinentry-qt".into()),
      origin: PinentryOrigin::Argvus,
    };
    assert_eq!(pinentry_choice(&qt), Some("qt"));
    assert_eq!(pinentry_choice(&Pinentry::default()), None);
  }

  #[test]
  fn expands_a_leading_tilde_only() {
    assert_eq!(expand_home("~/key.asc", "/home/u"), "/home/u/key.asc");
    assert_eq!(expand_home("~", "/home/u"), "/home/u");
    assert_eq!(expand_home("/tmp/key.asc", "/home/u"), "/tmp/key.asc");
    assert_eq!(expand_home("~other/key.asc", "/home/u"), "~other/key.asc");
  }

  #[test]
  fn the_default_form_asks_for_a_passphrase_and_cycles_algorithms() {
    let form = GpgForm::default();
    assert!(form.passphrase);
    assert_eq!(form.expire, "2y");
    assert_eq!(form.algorithm, KeyAlgorithm::Ed25519);
    assert_eq!(KeyAlgorithm::Ed25519.toggled(), KeyAlgorithm::Rsa4096);
    assert_eq!(KeyAlgorithm::Rsa4096.toggled(), KeyAlgorithm::Ed25519);
  }

  #[test]
  fn generation_with_a_passphrase_leaves_out_the_unprotected_flags() {
    let form = GpgForm {
      identity: " Alice <alice@example.com> ".into(),
      ..GpgForm::default()
    };
    let args = generate_args(&form);
    assert_eq!(
      args,
      [
        "generate",
        "Alice <alice@example.com>",
        "--algo",
        "ed25519",
        "--expire",
        "2y"
      ]
    );
  }

  #[test]
  fn generation_without_a_passphrase_asks_the_launcher_for_confirmation() {
    let form = GpgForm {
      identity: "Alice".into(),
      algorithm: KeyAlgorithm::Rsa4096,
      expire: "0".into(),
      passphrase: false,
    };
    let args = generate_args(&form);
    assert!(args.contains(&"--no-passphrase"));
    assert!(args.contains(&"--yes"));
    assert!(args.contains(&"rsa4096"));
  }

  #[test]
  fn refuses_a_pinentry_choice_outside_the_list_before_running_anything() {
    assert!(set_pinentry("tty").is_err());
    assert!(set_pinentry("--yes").is_err());
  }
}
