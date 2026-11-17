//! The pack shipped in `examples/plugins` must load and validate, so the docs
//! can point at a file that is known to work.

use roedit_plugins::PluginRegistry;
use std::path::PathBuf;

fn example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/plugins")
}

#[test]
fn shipped_example_pack_loads() {
    let mut registry = PluginRegistry::new();
    let errors = registry.load_dir(&example_dir());
    assert!(errors.is_empty(), "example pack failed to load: {errors:?}");
    assert_eq!(registry.packs().len(), 1);
    let pack = &registry.packs()[0];
    assert_eq!(pack.manifest.id, "roedit.starter");
    assert_eq!(pack.manifest.commands.len(), 3);
    assert!(
        pack.manifest.validate().is_empty(),
        "{:?}",
        pack.manifest.validate()
    );
}

#[test]
fn shipped_example_commands_execute_on_a_project() {
    let mut registry = PluginRegistry::new();
    assert!(registry.load_dir(&example_dir()).is_empty());
    let commands: Vec<String> = registry
        .packs()
        .iter()
        .flat_map(|p| p.manifest.commands.iter().map(|c| c.id.clone()))
        .collect();
    assert_eq!(commands.len(), 3);
    for id in commands {
        let mut project = roedit_core::project::Project::new("Example");
        let report = registry
            .execute(&id, &mut project)
            .unwrap_or_else(|err| panic!("{id} failed: {err}"));
        assert!(report.steps > 0, "{id} ran no steps");
    }
}
