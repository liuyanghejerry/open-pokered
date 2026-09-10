//! Zero-Rust game project loading: manifest → DSL compile → script registry.
//!
//! [`LoadedProject::load`] reads the `.dotzuki-editor.json` manifest, compiles
//! every DSL file under the manifest's `dsl_dirs` (in memory), registers the
//! compiled scenes with a [`ScriptLoader`], and keeps the storyline routing
//! table and a scene-name ↔ file-stem index for entry resolution.
//!
//! All file access goes through the [`ProjectFiles`] VFS: `load` is the
//! on-disk convenience ([`DiskFiles`]); [`load_with_files`](Self::load_with_files)
//! boots a project from any backend (e.g. an in-memory [`MemoryFiles`] on
//! WASM). DSL `source_path`s are project-relative POSIX paths in both cases
//! (`data/maps/<id>/script.scene`), which is what the runtime's scene ↔ map
//! matching keys on.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use dotzuki_engine_dsl::compiler::{compile_files, CompileReport, RouteEntry, DSL_EXTENSIONS};
use dotzuki_engine_dsl::loader::register_compiled;
use dotzuki_engine_script::loader::ScriptLoader;

use crate::manifest::Manifest;
use crate::map::RuntimeMap;
use crate::vfs::{join_path, DiskFiles, ProjectFiles};

/// Default maps directory under `dataRoot` when no `map` activity configures
/// `mapsDir`.
pub const DEFAULT_MAPS_DIR: &str = "maps";

/// The manifest filename at the project root.
const MANIFEST_FILE: &str = ".dotzuki-editor.json";

/// A fully loaded zero-Rust game project.
pub struct LoadedProject {
    /// The file backend every project read goes through.
    files: Arc<dyn ProjectFiles>,
    root: PathBuf,
    manifest: Manifest,
    data_root: PathBuf,
    /// `data_root` as a project-relative POSIX path (VFS key prefix).
    data_root_rel: String,
    gfx_root: Option<PathBuf>,
    /// `gfx_root` as a project-relative POSIX path, when configured.
    gfx_root_rel: Option<String>,
    scripts: ScriptLoader,
    report: CompileReport,
    /// `.scene` file stem → compiled scene name (`game_scene X`).
    stem_to_name: HashMap<String, String>,
    /// Compiled scene name → `.scene` file stem.
    name_to_stem: HashMap<String, String>,
}

impl LoadedProject {
    /// Load the project rooted at `root` (the directory containing
    /// `.dotzuki-editor.json`) from disk. Convenience for
    /// [`load_with_files`](Self::load_with_files) over a [`DiskFiles`].
    ///
    /// # Errors
    ///
    /// Fails on a missing/unparseable manifest or any DSL diagnostic.
    pub fn load(root: &Path) -> Result<Self> {
        Self::load_with_files(Arc::new(DiskFiles::new(root)))
    }

    /// Load the project from a [`ProjectFiles`] backend.
    ///
    /// The DSL is compiled in memory; any compiler diagnostic (unreadable
    /// file, compile failure, route conflict) aborts the load with an error
    /// listing every diagnostic — the same bar `dotzuki check` enforces.
    ///
    /// # Errors
    ///
    /// Fails on a missing/unparseable manifest or any DSL diagnostic.
    pub fn load_with_files(files: Arc<dyn ProjectFiles>) -> Result<Self> {
        let root: PathBuf = files.root().map(Path::to_path_buf).unwrap_or_default();
        let bytes = files.read(MANIFEST_FILE).map_err(|_| {
            anyhow::anyhow!(
                "no {MANIFEST_FILE} found in {} — not a jrpg game project",
                if root.as_os_str().is_empty() {
                    "<memory>".to_string()
                } else {
                    root.display().to_string()
                }
            )
        })?;
        let text =
            String::from_utf8(bytes).with_context(|| format!("{MANIFEST_FILE} is not UTF-8"))?;
        let manifest: Manifest = serde_json::from_str(&text)
            .with_context(|| format!("failed to parse {MANIFEST_FILE}"))?;

        let data_root_rel = join_path("", &manifest.data_root);
        let gfx_root_rel = manifest.gfx_root.as_deref().map(|g| join_path("", g));
        let data_root = root.join(&data_root_rel);
        let gfx_root = gfx_root_rel.as_ref().map(|g| root.join(g));

        let report = compile_project_dsl(files.as_ref(), &manifest);
        if !report.diagnostics.is_empty() {
            bail!(
                "DSL compile failed with {} diagnostic(s):\n  {}",
                report.diagnostics.len(),
                report.diagnostics.join("\n  ")
            );
        }

        let mut scripts = ScriptLoader::new();
        register_compiled(&mut scripts, &report);

        let (stem_to_name, name_to_stem) = stem_indexes(&report);

        Ok(Self {
            files,
            root,
            manifest,
            data_root,
            data_root_rel,
            gfx_root,
            gfx_root_rel,
            scripts,
            report,
            stem_to_name,
            name_to_stem,
        })
    }

    /// Recompile every DSL directory and swap the compiled scenes in place.
    ///
    /// On success the script registry, routing table and stem indexes are
    /// replaced wholesale — a scene currently mid-activation keeps running
    /// the JS it was started with; the next activation picks up the new
    /// source. On any compiler diagnostic the old scenes are kept and an
    /// error listing every diagnostic is returned (same bar as [`load`]).
    ///
    /// # Errors
    ///
    /// Fails when the recompile produces any diagnostic.
    ///
    /// [`load`]: Self::load
    pub fn recompile_scripts(&mut self) -> Result<()> {
        let report = compile_project_dsl(self.files.as_ref(), &self.manifest);
        if !report.diagnostics.is_empty() {
            bail!(
                "DSL recompile failed with {} diagnostic(s):\n  {}",
                report.diagnostics.len(),
                report.diagnostics.join("\n  ")
            );
        }

        let mut scripts = ScriptLoader::new();
        register_compiled(&mut scripts, &report);
        let (stem_to_name, name_to_stem) = stem_indexes(&report);

        self.scripts = scripts;
        self.report = report;
        self.stem_to_name = stem_to_name;
        self.name_to_stem = name_to_stem;
        Ok(())
    }

    /// The project's file backend.
    #[inline]
    pub fn files(&self) -> &Arc<dyn ProjectFiles> {
        &self.files
    }

    /// Project root directory (empty for a project without a disk root).
    #[inline]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The parsed `.dotzuki-editor.json` manifest.
    #[inline]
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Resolved data root (manifest `dataRoot` against the project root).
    #[inline]
    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    /// The data root as a project-relative POSIX path (VFS key prefix).
    #[inline]
    pub fn data_root_rel(&self) -> &str {
        &self.data_root_rel
    }

    /// Resolved graphics root (manifest `gfxRoot`), when configured.
    #[inline]
    pub fn gfx_root(&self) -> Option<&Path> {
        self.gfx_root.as_deref()
    }

    /// The graphics root as a project-relative POSIX path (the manifest's
    /// `gfxRoot`, default `"gfx"`).
    #[inline]
    pub fn gfx_root_rel(&self) -> String {
        self.gfx_root_rel
            .clone()
            .unwrap_or_else(|| "gfx".to_string())
    }

    /// Registry of compiled scene JS, keyed by scene name.
    #[inline]
    pub fn scripts(&self) -> &ScriptLoader {
        &self.scripts
    }

    /// The full DSL compile report.
    #[inline]
    pub fn report(&self) -> &CompileReport {
        &self.report
    }

    /// Storyline routing table `(map, npc/onEnter) → storyline`, collected
    /// from `@trigger` declarations across all scenes.
    #[inline]
    pub fn routes(&self) -> &[RouteEntry] {
        &self.report.routes
    }

    /// Compiled scene name for a `.scene` file stem (e.g. `"main"` →
    /// `"Main"`).
    #[inline]
    pub fn scene_name_for_stem(&self, stem: &str) -> Option<&str> {
        self.stem_to_name.get(stem).map(String::as_str)
    }

    /// `.scene` file stem for a compiled scene name.
    #[inline]
    pub fn stem_for_scene_name(&self, name: &str) -> Option<&str> {
        self.name_to_stem.get(name).map(String::as_str)
    }

    /// The maps directory as a project-relative POSIX path: the `map`
    /// activity's `mapsDir` (dataRoot-relative), default `maps`.
    pub fn maps_dir_rel(&self) -> String {
        let dir = self
            .manifest
            .activities
            .iter()
            .find(|a| a.kind == "map")
            .and_then(|a| a.config.get("mapsDir"))
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_MAPS_DIR);
        join_path(&self.data_root_rel, dir)
    }

    /// Directory holding the per-map subdirectories (disk form of
    /// [`maps_dir_rel`](Self::maps_dir_rel)).
    pub fn maps_dir(&self) -> PathBuf {
        self.root.join(self.maps_dir_rel())
    }

    /// Sorted ids of all map directories under [`maps_dir`](Self::maps_dir).
    pub fn map_ids(&self) -> Vec<String> {
        let prefix = format!("{}/", self.maps_dir_rel());
        let mut ids: Vec<String> = self
            .files
            .list(&self.maps_dir_rel())
            .iter()
            .filter_map(|p| {
                // Only files INSIDE a subdirectory name a map (a direct file
                // under the maps dir is not a map).
                let rest = p.strip_prefix(&prefix)?;
                rest.contains('/')
                    .then(|| rest.split('/').next().unwrap().to_string())
            })
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Map to spawn on: `game.entryMap`, or the first map directory (sorted)
    /// under the maps dir.
    ///
    /// # Errors
    ///
    /// Fails when no `entryMap` is configured and no map directories exist.
    pub fn entry_map(&self) -> Result<String> {
        if let Some(entry) = self
            .manifest
            .game
            .as_ref()
            .and_then(|g| g.entry_map.as_deref())
        {
            return Ok(entry.to_string());
        }
        self.map_ids().into_iter().next().with_context(|| {
            format!(
                "no game.entryMap in the manifest and no maps under {}",
                self.maps_dir().display()
            )
        })
    }

    /// Scene to boot into: `game.entryScene` (a `.scene` file stem) resolved
    /// to its compiled scene name, or the first compiled scene sorted by
    /// source path.
    ///
    /// # Errors
    ///
    /// Fails when `entryScene` names a stem that compiled to nothing, or
    /// when the project has no scenes at all.
    pub fn entry_scene_name(&self) -> Result<&str> {
        if let Some(stem) = self
            .manifest
            .game
            .as_ref()
            .and_then(|g| g.entry_scene.as_deref())
        {
            return self
                .scene_name_for_stem(stem)
                .with_context(|| format!("game.entryScene '{stem}' did not compile to any scene"));
        }
        self.report
            .scenes
            .iter()
            .min_by(|a, b| a.2.cmp(&b.2))
            .map(|(name, _, _)| name.as_str())
            .context("project compiled no scenes; nothing to boot into")
    }

    /// Load a map by id from this project's maps dir.
    pub fn load_map(&self, map_id: &str) -> Result<RuntimeMap> {
        RuntimeMap::load_with_files(self.files.as_ref(), &self.maps_dir_rel(), map_id)
    }

    /// Directory holding the records of data table `table_id` (a table id
    /// from the data activity's `config.tables[]`), resolved against the
    /// project root. `None` when the id names no declared table.
    pub fn table_dir(&self, table_id: &str) -> Option<PathBuf> {
        self.table_dir_rel(table_id).map(|rel| self.root.join(rel))
    }

    /// The record directory of data table `table_id` as a project-relative
    /// POSIX path (the VFS form of [`table_dir`](Self::table_dir)). `None`
    /// when the id names no declared table.
    pub fn table_dir_rel(&self, table_id: &str) -> Option<String> {
        self.manifest
            .data_table(table_id)
            .map(|t| join_path(&self.data_root_rel, &t.dir))
    }
}

/// Compile every DSL file under the manifest's DSL dirs through the VFS:
/// `list` discovers, `read` loads, `compile_files` compiles in memory.
/// Source paths are project-relative POSIX paths — the same shape
/// `compile_dirs` produces for a relative project root, and what the
/// runtime's scene ↔ map matching expects. Unreadable/non-UTF-8 files
/// become diagnostics (the same bar `dotzuki check` enforces).
fn compile_project_dsl(files: &dyn ProjectFiles, manifest: &Manifest) -> CompileReport {
    let mut dsl_files: Vec<(String, String, String)> = Vec::new();
    let mut read_errors: Vec<String> = Vec::new();
    for dir in manifest.dsl_dirs_rel() {
        for path in files.list(&dir) {
            // Mirror the disk scanner: skip hidden files/dirs, node_modules
            // and target anywhere in the path.
            if path
                .split('/')
                .any(|c| c.starts_with('.') || c == "node_modules" || c == "target")
            {
                continue;
            }
            let ext = path.rsplit('.').next().unwrap_or("");
            if !DSL_EXTENSIONS.contains(&ext) {
                continue;
            }
            match files.read(&path) {
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(content) => dsl_files.push((ext.to_string(), path, content)),
                    Err(_) => read_errors.push(format!("Failed to read {path}: not UTF-8")),
                },
                Err(e) => read_errors.push(format!("Failed to read {path}: {e:#}")),
            }
        }
    }
    let mut report = compile_files(&dsl_files, None);
    report.diagnostics.extend(read_errors);
    report
}

/// `.scene` file stem ↔ compiled scene name indexes for a compile report.
fn stem_indexes(report: &CompileReport) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut stem_to_name = HashMap::new();
    let mut name_to_stem = HashMap::new();
    for (name, _js, source_path) in &report.scenes {
        let stem = Path::new(source_path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        stem_to_name.insert(stem.clone(), name.clone());
        name_to_stem.insert(name.clone(), stem);
    }
    (stem_to_name, name_to_stem)
}
