use std::collections::{BTreeSet, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::Context as _;
use bon::bon;
use getset::{CopyGetters, Getters};
use libloading::Library;
use stiles_core::Lint;
use stiles_types::plugin::{AbiVersion, LintPassFactory, PluginDeclaration, construct};
use stiles_types::{BoxedLintPass, Language, LintPass, Panic, RuleId, RuleOptions};

use self::handshake::AbiIdentity;
use crate::config::{GitLintSource, LintSource, StilesConfig};

mod abi_tag;
mod artifact;
mod cache;
mod digest;
mod git_source;
mod handshake;
mod prebuilt;
mod stiles_patch;

pub(crate) use self::abi_tag::AbiTag;
pub(crate) use self::prebuilt::PrebuiltNotice;

/// The lint passes loaded from the project's configured custom lint crates
///
/// Loading happens once per run, before the file walk. Stiles compiles each
/// configured path with the cargo of the user, and opens the dynamic
/// library that the build produced. It checks the declaration of that
/// library against the ABI identity of this binary before anything in it
/// runs. What survives is a set of factories. Passes are stateful, and the
/// check command constructs a fresh set for every file.
///
/// A load failure is fatal rather than recoverable. Configuration that
/// silently does nothing looks exactly like configuration that works.
/// `--keep-going` governs the walk errors of single files, not a broken
/// setup.
pub(crate) struct CustomLints {
    lints: Vec<LoadedLint>,
}

#[bon]
impl CustomLints {
    /// Compiles, loads, and validates every configured custom lint crate
    ///
    /// A git source may come from prebuilt libraries. Whatever the reader
    /// should hear about that lookup goes into `notices`, which the caller
    /// keeps whether or not the load succeeds.
    ///
    /// # Errors
    ///
    /// Returns an error if a configured source cannot be resolved, is
    /// configured twice, or fails to build. Returns an error too if it does
    /// not export a plugin declaration, fails the ABI handshake, or exports
    /// no lints. It also
    /// returns one if a lint declares no rule, or if two lints declare one
    /// rule for one language.
    #[builder]
    pub(crate) fn load(
        config: &StilesConfig,
        notices: &mut Vec<PrebuiltNotice>,
    ) -> anyhow::Result<Self> {
        let host = AbiIdentity::host();
        let mut lints = Vec::new();

        for Resolved {
            directory,
            contents,
        } in configured_sources(config, &AbiTag::host(), notices)
            .context("failed to resolve the configured lint sources")?
        {
            let loaded = match contents {
                Contents::Sources(locking) => {
                    load_sources(&directory, locking, config.stiles_source(), &host).with_context(
                        || format!("failed to load the custom lints at {}", directory.display()),
                    )?
                }
                Contents::Libraries => load_prebuilt(&directory, &host).with_context(|| {
                    format!(
                        "failed to load the prebuilt lints at {}",
                        directory.display()
                    )
                })?,
            };

            lints.extend(loaded.lints);
        }

        refuse_shared_rules(&lints).context("failed to check the rules that the lints declare")?;

        Ok(Self { lints })
    }

    /// Constructs one fresh pass per loaded custom lint
    ///
    /// Every pass runs, and a rule a project turned off is dropped from the
    /// report rather than never looked for. One pass may report several
    /// rules, and only some of them may be off.
    ///
    /// Every pass is configured before it is returned. Each one reads only
    /// the options of the rules it declares, so a pass never sees an entry
    /// meant for another rule. The walk holds each pass to those rules.
    ///
    /// # Errors
    ///
    /// Returns an error if a pass panics while it is constructed or while
    /// it reads the table. The plugin caught the panic at its boundary and
    /// handed it back. A pass that cannot be built or configured is a
    /// broken setup rather than a file to skip.
    pub(crate) fn instantiate(&self, options: &RuleOptions) -> anyhow::Result<Vec<Lint<'static>>> {
        self.lints
            .iter()
            .map(|lint| {
                let constructed: Result<BoxedLintPass, Panic> = construct(&lint.factory).into();
                let pass = constructed.context("a lint pass panicked while it was constructed")?;
                let mut pass: Box<dyn LintPass> = Box::new(pass);
                let configured: Result<(), Panic> =
                    pass.configure(&options.for_rules(&lint.rules)).into();
                configured.context("a lint pass panicked while reading the project's options")?;
                Ok(Lint::builder().pass(pass).rules(lint.rules.clone()).build())
            })
            .collect()
    }

    /// Returns every rule the loaded plugins declare
    ///
    /// This is what a configured rule name is checked against. A plugin
    /// declares its rules, so stiles knows them all before it walks a
    /// single file. A name that matches none of them is a mistake that it
    /// can report, not a filter that quietly admits everything.
    ///
    pub(crate) fn declared(&self) -> BTreeSet<String> {
        self.lints
            .iter()
            .flat_map(|lint| &lint.rules)
            .map(|rule| rule.as_str().to_owned())
            .collect()
    }

    /// Returns every declared rule with the languages it runs on, by name
    ///
    /// One rule may run on several languages, each through a lint of its
    /// own, so a rule appears once with every language it covers.
    pub(crate) fn rules(&self) -> Vec<DeclaredRule> {
        let mut rules: Vec<DeclaredRule> = Vec::new();

        for lint in &self.lints {
            for rule in &lint.rules {
                match rules.iter_mut().find(|known| known.rule == *rule) {
                    Some(known) => known.languages.push(lint.language),
                    None => rules.push(DeclaredRule {
                        rule: *rule,
                        languages: vec![lint.language],
                    }),
                }
            }
        }

        rules.sort_by(|one, other| one.rule.as_str().cmp(other.rule.as_str()));
        rules
    }
}

/// A rule a loaded plugin declares, and the languages it runs on
#[derive(Clone, Eq, PartialEq, Hash, Debug, CopyGetters, Getters)]
pub(crate) struct DeclaredRule {
    /// The id of the rule
    #[getset(get_copy = "pub")]
    rule: RuleId,

    /// The languages a lint reports the rule on, in load order
    #[getset(get = "pub")]
    languages: Vec<Language>,
}

/// One lint a plugin exported, as stiles keeps it
///
/// The library it came from stays with it, so an error about the lint can
/// say where to look.
struct LoadedLint {
    factory: LintPassFactory,
    language: Language,
    rules: Vec<RuleId>,
    library: PathBuf,
}

/// Refuses two lints that declare one rule for one language
///
/// Stiles routes options, switches, and listings by rule id and language.
/// Two lints under one id on one language would split each of those
/// between them. The cause is almost always a rule that was copied, and
/// renamed in one place only. The check spans every plugin, because two
/// repositories of rules can collide as easily as one.
///
/// # Errors
///
/// Returns an error that names the rule, the language, and both libraries.
fn refuse_shared_rules(lints: &[LoadedLint]) -> anyhow::Result<()> {
    for (index, lint) in lints.iter().enumerate() {
        for other in &lints[index + 1..] {
            if lint.language != other.language {
                continue;
            }
            let Some(shared) = lint.rules.iter().find(|rule| other.rules.contains(rule)) else {
                continue;
            };
            anyhow::bail!(
                "two lints declare `{shared}` for {}: one in {}, one in {}. A rule id names one \
                 lint per language, so rename one of them",
                lint.language,
                lint.library.display(),
                other.library.display(),
            );
        }
    }

    Ok(())
}

/// A resolved lint source, ready to load
///
/// Resolution loses the difference between a path and a repository. Two
/// things survive it: a directory, and whether stiles has to compile
/// what is in that directory.
#[derive(Clone, Eq, PartialEq, Debug)]
struct Resolved {
    directory: PathBuf,
    contents: Contents,
}

/// What a resolved directory holds
///
/// Both kinds end at the same place, which is a set of dynamic libraries
/// that pass the handshake. They differ in who compiled them. Stiles
/// compiles the first kind from cargo packages in the directory. A
/// publisher compiled the second kind.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Contents {
    /// Cargo packages, which stiles builds before it loads them
    Sources(Locking),

    /// Dynamic libraries, which stiles loads as they are
    Libraries,
}

/// Whether a build may change the lockfile it finds
///
/// A path source belongs to the person who runs stiles, and an update of
/// its lockfile by cargo is ordinary. A git source is pinned to a commit.
/// That pin is only worth as much as the dependency versions behind it. Its
/// build therefore refuses to resolve anything new to the committed
/// lockfile.
///
/// A patched build is the exception. Replacing the stiles a package
/// names changes the graph the lockfile settled, so cargo has to resolve
/// again and `--locked` would refuse the build.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Locking {
    Locked,
    Unlocked,
}

/// Resolves and validates the configured lint sources before any build runs
///
/// Everything is resolved up front. A typo in the second entry therefore
/// surfaces before the potentially long compilation of the first entry. A
/// source configured twice is caught however it was spelled. A git source
/// reaches the network here, so this also happens before the builds. A
/// network failure should not arrive after five minutes of compilation.
///
/// Stiles looks for prebuilt libraries before it takes a git source. It
/// fetches and compiles the source whenever it finds none. A project
/// that no publisher builds for therefore behaves as it did before.
///
/// # Errors
///
/// Returns an error if an entry does not exist, is not a directory, holds
/// no `Cargo.toml`, or cannot be fetched. Returns an error too if it
/// resolves to the same directory as an earlier entry.
fn configured_sources(
    config: &StilesConfig,
    tag: &AbiTag,
    notices: &mut Vec<PrebuiltNotice>,
) -> anyhow::Result<Vec<Resolved>> {
    let mut sources = Vec::new();
    let mut seen = HashSet::new();

    for entry in config.lints() {
        let (directory, contents) = match entry {
            LintSource::Path(path) => (
                path.resolve(config.root()),
                Contents::Sources(Locking::Unlocked),
            ),
            LintSource::Git(git) => resolve_git(git, tag, notices)
                .with_context(|| format!("failed to resolve the lint source {entry}"))?,
        };

        anyhow::ensure!(
            directory.exists(),
            "the lint source {entry} resolves to {}, which does not exist",
            directory.display()
        );
        anyhow::ensure!(
            directory.is_dir(),
            "the lint source {entry} resolves to {}, which is not a directory",
            directory.display()
        );

        let directory = std::fs::canonicalize(&directory)
            .with_context(|| format!("failed to resolve the lint source {entry}"))?;

        match contents {
            Contents::Sources(_) => anyhow::ensure!(
                directory.join("Cargo.toml").is_file(),
                "the lint source {entry} holds no Cargo.toml"
            ),
            Contents::Libraries => {}
        }

        anyhow::ensure!(
            seen.insert(directory.clone()),
            "the lint source {entry} resolves to {}, which an earlier entry already configured",
            directory.display()
        );

        sources.push(Resolved {
            directory,
            contents,
        });
    }

    Ok(sources)
}

/// Resolves a git entry, preferring whatever the machine already holds
///
/// The order is by what each answer costs. Prebuilt libraries stiles
/// already unpacked are the cheapest and win outright. A checkout that is
/// already there comes next. Stiles has to compile it, but cargo has
/// compiled it before. A question to a release API first would put a
/// network request in front of a run that needs none. Every check on a
/// train would then stop working. Only a machine that holds neither asks
/// anyone.
///
/// A project with a warm checkout therefore keeps compiling it, even
/// after its rules start to publish archives. A person moves the pin or
/// clears the cache to pick those up.
///
/// # Errors
///
/// Returns an error if no cache location can be determined, or if the
/// source has to be fetched and cannot be.
fn resolve_git(
    source: &GitLintSource,
    tag: &AbiTag,
    notices: &mut Vec<PrebuiltNotice>,
) -> anyhow::Result<(PathBuf, Contents)> {
    if let Some(directory) =
        self::prebuilt::cached(source, tag).context("failed to look for cached prebuilt lints")?
    {
        return Ok((directory, Contents::Libraries));
    }

    if let Some(directory) =
        self::git_source::cached(source).context("failed to look for a cached checkout")?
    {
        return Ok((directory, Contents::Sources(Locking::Locked)));
    }

    if let Some(directory) =
        self::prebuilt::fetch(source, tag, notices).context("failed to fetch prebuilt lints")?
    {
        return Ok((directory, Contents::Libraries));
    }

    Ok((
        self::git_source::checkout(source).context("failed to check out the lint source")?,
        Contents::Sources(Locking::Locked),
    ))
}

/// Builds the packages at `directory` and loads the lints they export
///
/// A directory may hold one package or a workspace of them, so every
/// dynamic library the build produced is loaded, each with its own
/// handshake.
fn load_sources(
    directory: &Path,
    locking: Locking,
    stiles_source: Option<&Path>,
    host: &AbiIdentity,
) -> anyhow::Result<Loaded> {
    let libraries = build(directory, locking, stiles_source)
        .with_context(|| format!("failed to build the lints at {}", directory.display()))?;

    load_libraries(&libraries, host)
}

/// Loads the lints exported by the prebuilt libraries at `directory`
///
/// Each library completes the same handshake as one stiles compiled
/// itself, and a failure fails the run. The tag stiles asked under
/// covers what the handshake compares. A library that fails therefore
/// carries a tag that misdescribes it, and a quiet compile would leave
/// that wrong for everyone who trusts the tag.
///
/// # Errors
///
/// Returns an error if the directory cannot be read, if it holds no
/// library, or if any library fails to load.
fn load_prebuilt(directory: &Path, host: &AbiIdentity) -> anyhow::Result<Loaded> {
    let libraries = self::prebuilt::libraries(directory)
        .with_context(|| format!("failed to list the libraries at {}", directory.display()))?;

    anyhow::ensure!(
        !libraries.is_empty(),
        "the directory holds no dynamic library; delete it and run stiles again to replace it"
    );

    load_libraries(&libraries, host)
}

/// Completes the handshake with each library and collects what each exports
fn load_libraries(libraries: &[PathBuf], host: &AbiIdentity) -> anyhow::Result<Loaded> {
    let mut all = Loaded::default();

    for library in libraries {
        let loaded = load_library(library, host)
            .with_context(|| format!("failed to load {}", library.display()))?;
        all.lints.extend(loaded.lints);
    }

    Ok(all)
}

/// What one library, or a directory of them, contributed
#[derive(Default)]
struct Loaded {
    lints: Vec<LoadedLint>,
}

/// Compiles the packages at `directory` and returns every library built
///
/// The build inherits stderr, so compiler errors and progress render to
/// the terminal exactly as they would for a direct `cargo build`. Stdout
/// carries the JSON messages that the artifact search reads. The build
/// uses the release profile, because the plugin then runs over every file
/// of every check.
///
/// # Errors
///
/// Returns an error if cargo cannot be run, exits unsuccessfully, or
/// produces no dynamic library.
fn build(
    directory: &Path,
    locking: Locking,
    stiles_source: Option<&Path>,
) -> anyhow::Result<Vec<PathBuf>> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));

    let mut command = Command::new(&cargo);
    command.args([
        "build",
        "--release",
        "--message-format=json-render-diagnostics",
    ]);

    match (locking, stiles_source) {
        (Locking::Locked, None) => {
            command.arg("--locked");
        }
        (Locking::Locked, Some(_)) | (Locking::Unlocked, _) => {}
    }

    if let Some(source) = stiles_source {
        let patches = self::stiles_patch::load_patches(&cargo, directory)
            .context("failed to find the stiles crates that the lints depend on")?;
        for patch in patches {
            command.arg("--config").arg(patch.config(source));
        }
    }

    let output = command
        .current_dir(directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .context("failed to run cargo build")?;

    anyhow::ensure!(output.status.success(), "cargo build failed");

    let stdout = String::from_utf8(output.stdout).context("cargo build wrote invalid UTF-8")?;

    self::artifact::cdylib_artifacts(&stdout, directory)
}

/// Opens the built library, performs the handshake, and collects factories
///
/// The loader never forms a `&PluginDeclaration`. A plugin built against
/// an older protocol exports a shorter static. A reference asserts that
/// the whole of the current struct is there, with a valid value in every
/// field. Both claims are false for such a plugin. `load` is a function
/// pointer, so the compiler may assume it is not null. That
/// is undefined behavior whether or not the field is ever read.
///
/// So each field is read through a raw pointer at its own offset. A field
/// is only projected once [`PluginDeclaration::abi_version`] says that the
/// plugin exported it. This is what makes an appended field cheap. The
/// offsets of a `#[repr(C)]` struct are knowable per version. No step of
/// the read depends on the agreement of the plugin about the size of the
/// struct. Stiles is before 1.0, where the version of a plugin has to equal
/// the version of this binary. Every field that a plugin exports is
/// therefore one this version knows. The read stays field by field,
/// because from 1.0 an older minor loads and ends sooner.
///
/// The loader deliberately leaks the library. The factories that it hands
/// over point into the image of the library. So does the `&'static str`
/// inside every [`RuleId`] that a plugin lint mints. An unload would leave
/// dangling references behind values that outlive this function. The leak
/// is bounded by the number of configured plugins in a short-lived process.
///
/// # Errors
///
/// Returns an error if the library cannot be opened, exports no plugin
/// declaration, fails the ABI handshake, or exports no lints.
///
/// [`RuleId`]: stiles_types::RuleId
fn load_library(library_path: &Path, host: &AbiIdentity) -> anyhow::Result<Loaded> {
    // SAFETY: opening a library runs its initializers, which is unsound
    // for any library whose initializers are. Stiles opens only what a
    // project's configuration names, which the project already trusts to
    // run as a lint.
    let library = unsafe { Library::new(library_path) }
        .with_context(|| format!("failed to open {}", library_path.display()))?;

    let declaration =
        // SAFETY: the symbol's type is checked before the value is used.
        // `abi_version` reads the first field, and the handshake refuses
        // the library unless the whole declaration matches this binary.
        unsafe { library.get::<*const PluginDeclaration>(b"stiles_plugin_declaration\0") }
            .context(
                "the library is not a stiles lint plugin; export its lints with \
             stiles_plugin::export_lints!",
            )?;
    let declaration: *const PluginDeclaration = *declaration;

    // SAFETY: `declaration` is the address `dlsym` gave for the loaded
    // library's `stiles_plugin_declaration` static.
    let plugin_abi_version = unsafe { abi_version(declaration) };
    if !host.abi_version().accepts(plugin_abi_version) {
        return Err(self::handshake::HandshakeMismatch::AbiVersion {
            plugin: plugin_abi_version,
            host: host.abi_version(),
        }
        .into());
    }

    // SAFETY: the protocol is one this binary knows, so the plugin lays
    // both fingerprints out where this binary reads them.
    let types_fingerprint = unsafe { (&raw const (*declaration).types_fingerprint).read() };
    // SAFETY: as for `types_fingerprint` above.
    let language_fingerprint = unsafe { (&raw const (*declaration).language_fingerprint).read() };
    let plugin = AbiIdentity::builder()
        .abi_version(plugin_abi_version)
        .types_fingerprint(types_fingerprint)
        .language_fingerprint(language_fingerprint)
        .build();
    self::handshake::validate(host, &plugin).context("failed the handshake with the plugin")?;

    // SAFETY: the handshake passed, so the plugin lays the declaration
    // out the way this binary does, and `load` holds a function this
    // binary can call.
    let load = unsafe { (&raw const (*declaration).load).read() };
    let loaded: Result<_, Panic> = load().into();
    let loaded = loaded.context("the plugin panicked while listing what it exports")?;
    let lints: Vec<LoadedLint> = loaded
        .lints
        .into_iter()
        .map(|lint| LoadedLint {
            factory: lint.factory,
            language: lint.language,
            rules: lint.rules.into_iter().collect(),
            library: library_path.to_path_buf(),
        })
        .collect();

    anyhow::ensure!(
        !lints.is_empty(),
        "the plugin exported no lints; a plugin that does nothing looks exactly like one that \
         works, so this is treated as a mistake"
    );
    anyhow::ensure!(
        lints.iter().all(|lint| !lint.rules.is_empty()),
        "the plugin exported a lint that declares no rules; stiles refuses every diagnostic \
         such a lint reports, so give it a `DeclaresRules` implementation that names them"
    );

    std::mem::forget(library);

    Ok(Loaded { lints })
}

/// Reads the protocol version at the head of a plugin declaration
///
/// This is the one field a plugin of any vintage can be trusted to hold,
/// because it sits at offset zero of a `#[repr(C)]` struct. Only a
/// matching version establishes everything past it, the size of the struct
/// included. The read therefore goes through a raw pointer, not a
/// reference to the whole declaration.
///
/// # Safety
///
/// `declaration` must be the address of a loaded library's
/// `stiles_plugin_declaration` static, which is at least as large as an
/// [`AbiVersion`]. Every protocol writes one there, and `export_lints!`
/// writes every declaration stiles loads.
unsafe fn abi_version(declaration: *const PluginDeclaration) -> AbiVersion {
    // SAFETY: every protocol puts `abi_version` first, and it is eight
    // bytes wide; the `abi_version_sits_at_offset_zero` and
    // `major_sits_first_and_the_pair_is_eight_bytes` tests pin both. Any
    // bit pattern of those bytes is a valid `AbiVersion`, which holds two
    // `u32`.
    unsafe { declaration.cast::<AbiVersion>().read_unaligned() }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use kawauso_project::project::ProjectRoot;

    use super::*;
    use crate::config::LintPath;

    fn config_with(root: &Path, lints: Vec<LintPath>) -> StilesConfig {
        let lints = lints.into_iter().map(LintSource::Path).collect();

        StilesConfig::builder()
            .root(ProjectRoot::new(root.to_path_buf()))
            .lints(lints)
            .build()
    }

    fn directories(config: &StilesConfig) -> anyhow::Result<Vec<PathBuf>> {
        let sources = configured_sources(config, &AbiTag::host(), &mut Vec::new())
            .context("failed to resolve the configured lint sources")?;

        Ok(sources
            .into_iter()
            .map(|Resolved { directory, .. }| directory)
            .collect())
    }

    fn package(root: &Path, name: &str) -> PathBuf {
        let directory = root.join(name);
        std::fs::create_dir_all(&directory).expect("package directory should be created");
        std::fs::write(directory.join("Cargo.toml"), "[package]\n").expect("manifest");
        directory
    }

    #[test]
    fn abi_version_reads_a_declaration_that_ends_after_the_version() {
        #[repr(C)]
        struct Truncated {
            abi_version: AbiVersion,
        }
        let truncated = Truncated {
            abi_version: AbiVersion { major: 3, minor: 4 },
        };

        // SAFETY: `Truncated` holds the one field `abi_version` reads,
        // which is what this test pins about a shorter declaration.
        let version = unsafe { abi_version((&raw const truncated).cast::<PluginDeclaration>()) };

        assert_eq!(version, AbiVersion { major: 3, minor: 4 });
    }

    #[test]
    fn configured_sources_leaves_a_path_source_unlocked() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        package(root.path(), "no_todo");
        let config = config_with(root.path(), vec![LintPath::new("no_todo")]);

        let sources =
            configured_sources(&config, &AbiTag::host(), &mut Vec::new()).expect("should resolve");

        assert_eq!(
            sources.first().map(|source| source.contents),
            Some(Contents::Sources(Locking::Unlocked)),
            "a lockfile on disk belongs to the person running stiles"
        );
    }

    #[test]
    fn configured_sources_rejects_a_duplicate_entry() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        package(root.path(), "no_todo");
        let config = config_with(
            root.path(),
            vec![LintPath::new("no_todo"), LintPath::new("./no_todo")],
        );

        let error = directories(&config).expect_err("should fail");

        assert!(
            format!("{error:#}").contains("already configured"),
            "unexpected: {error:#}"
        );
    }

    #[test]
    fn configured_sources_rejects_a_file_entry() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        std::fs::write(root.path().join("lint.rs"), "").expect("file should be written");
        let config = config_with(root.path(), vec![LintPath::new("lint.rs")]);

        let error = directories(&config).expect_err("should fail");

        assert!(
            format!("{error:#}").contains("not a directory"),
            "unexpected: {error:#}"
        );
    }

    #[test]
    fn configured_sources_rejects_a_missing_entry() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        let config = config_with(root.path(), vec![LintPath::new("absent")]);

        let error = directories(&config).expect_err("should fail");

        assert!(
            format!("{error:#}").contains("does not exist"),
            "unexpected: {error:#}"
        );
    }

    #[test]
    fn configured_sources_rejects_an_entry_without_manifest() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        std::fs::create_dir(root.path().join("empty")).expect("directory should be created");
        let config = config_with(root.path(), vec![LintPath::new("empty")]);

        let error = directories(&config).expect_err("should fail");

        assert!(
            format!("{error:#}").contains("no Cargo.toml"),
            "unexpected: {error:#}"
        );
    }

    #[test]
    fn configured_sources_resolves_entries_in_order() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        let first = package(root.path(), "first");
        let second = package(root.path(), "second");
        let config = config_with(
            root.path(),
            vec![LintPath::new("first"), LintPath::new("second")],
        );

        let resolved = directories(&config).expect("should resolve");

        assert_eq!(
            resolved,
            vec![
                std::fs::canonicalize(first).expect("should resolve"),
                std::fs::canonicalize(second).expect("should resolve"),
            ]
        );
    }

    #[test]
    fn load_with_no_configured_lints_yields_no_passes() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        let config = config_with(root.path(), Vec::new());

        let lints = CustomLints::load()
            .config(&config)
            .notices(&mut Vec::new())
            .call()
            .expect("should load");

        let passes = lints
            .instantiate(&RuleOptions::default())
            .expect("nothing to configure");

        assert!(passes.is_empty());
    }

    /// Describes a lint whose factory is never called
    fn loaded_lint(language: Language, rules: &[&'static str], library: &str) -> LoadedLint {
        LoadedLint {
            factory: stiles_types::plugin::factory(|| {
                Panic::catch(|| -> BoxedLintPass { unreachable!("never built") }).into()
            }),
            language,
            rules: rules.iter().map(|rule| RuleId::new(rule)).collect(),
            library: PathBuf::from(library),
        }
    }

    #[test]
    fn refuse_shared_rules_with_one_rule_on_two_languages_succeeds() {
        let lints = [
            loaded_lint(Language::Rust, &["lint.a"], "rust.so"),
            loaded_lint(Language::TypeScript, &["lint.a"], "typescript.so"),
        ];

        let refused = refuse_shared_rules(&lints);

        assert!(refused.is_ok());
    }

    #[test]
    fn refuse_shared_rules_with_one_rule_twice_on_one_language_names_both_libraries() {
        let lints = [
            loaded_lint(Language::Rust, &["lint.a", "lint.b"], "first.so"),
            loaded_lint(Language::Rust, &["lint.b"], "second.so"),
        ];

        let error = refuse_shared_rules(&lints).expect_err("should refuse");

        let message = error.to_string();
        let missing: Vec<&str> = ["`lint.b`", "first.so", "second.so"]
            .into_iter()
            .filter(|part| !message.contains(part))
            .collect();
        assert_eq!(missing, Vec::<&str>::new(), "{message}");
    }

    #[test]
    fn rules_lists_each_rule_once_with_its_languages() {
        let lints = CustomLints {
            lints: vec![
                loaded_lint(Language::Rust, &["lint.b", "lint.a"], "rust.so"),
                loaded_lint(Language::TypeScript, &["lint.a"], "typescript.so"),
            ],
        };

        let rules = lints.rules();

        assert_eq!(
            rules,
            [
                DeclaredRule {
                    rule: RuleId::new("lint.a"),
                    languages: vec![Language::Rust, Language::TypeScript],
                },
                DeclaredRule {
                    rule: RuleId::new("lint.b"),
                    languages: vec![Language::Rust],
                },
            ]
        );
    }

    #[test]
    fn trait_send_contents() {
        fn assert_send<T: Send>() {}
        assert_send::<Contents>();
    }

    #[test]
    fn trait_send_custom_lints() {
        fn assert_send<T: Send>() {}
        assert_send::<CustomLints>();
    }

    #[test]
    fn trait_send_declared_rule() {
        fn assert_send<T: Send>() {}
        assert_send::<DeclaredRule>();
    }

    #[test]
    fn trait_send_loaded() {
        fn assert_send<T: Send>() {}
        assert_send::<Loaded>();
    }

    #[test]
    fn trait_send_loaded_lint() {
        fn assert_send<T: Send>() {}
        assert_send::<LoadedLint>();
    }

    #[test]
    fn trait_send_locking() {
        fn assert_send<T: Send>() {}
        assert_send::<Locking>();
    }

    #[test]
    fn trait_send_resolved() {
        fn assert_send<T: Send>() {}
        assert_send::<Resolved>();
    }

    #[test]
    fn trait_sync_contents() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Contents>();
    }

    #[test]
    fn trait_sync_custom_lints() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<CustomLints>();
    }

    #[test]
    fn trait_sync_declared_rule() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DeclaredRule>();
    }

    #[test]
    fn trait_sync_loaded() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Loaded>();
    }

    #[test]
    fn trait_sync_loaded_lint() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<LoadedLint>();
    }

    #[test]
    fn trait_sync_locking() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Locking>();
    }

    #[test]
    fn trait_sync_resolved() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Resolved>();
    }

    #[test]
    fn trait_unpin_contents() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Contents>();
    }

    #[test]
    fn trait_unpin_custom_lints() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<CustomLints>();
    }

    #[test]
    fn trait_unpin_declared_rule() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<DeclaredRule>();
    }

    #[test]
    fn trait_unpin_loaded() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Loaded>();
    }

    #[test]
    fn trait_unpin_loaded_lint() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<LoadedLint>();
    }

    #[test]
    fn trait_unpin_locking() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Locking>();
    }

    #[test]
    fn trait_unpin_resolved() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<Resolved>();
    }
}
