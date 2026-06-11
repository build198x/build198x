//! Internal-consistency checks over every machine in the spec.
//!
//! These tests know nothing about specific hardware facts (those live in
//! `source_facts.rs`); they enforce the structural invariants the converter
//! relies on, across all machines at once.

use mediaspec::{ConstraintRule, PaletteModel, machine, machines};

/// Machine ids are unique and round-trip through the by-id accessor.
#[test]
fn machine_ids_unique_and_resolvable() {
    let all = machines();
    for (i, m) in all.iter().enumerate() {
        assert!(
            all.iter().skip(i + 1).all(|other| other.id != m.id),
            "duplicate machine id {:?}",
            m.id
        );
        let found = machine(m.id).unwrap_or_else(|| panic!("machine({:?}) not found", m.id));
        assert!(
            std::ptr::eq(found, *m),
            "machine({:?}) resolved wrongly",
            m.id
        );
    }
    assert!(machine("no-such-machine").is_none());
}

/// Mode names are unique within each machine.
#[test]
fn mode_names_unique_per_machine() {
    for m in machines() {
        for (i, mode) in m.modes.iter().enumerate() {
            assert!(
                m.modes
                    .iter()
                    .skip(i + 1)
                    .all(|other| other.name != mode.name),
                "{}: duplicate mode name {:?}",
                m.id,
                mode.name
            );
        }
    }
}

/// Every mode has nonzero paper dimensions and nonzero pixel-aspect
/// components.
#[test]
fn geometry_components_nonzero() {
    for m in machines() {
        for mode in m.modes {
            assert!(mode.paper_width > 0, "{}/{}", m.id, mode.name);
            assert!(mode.paper_height > 0, "{}/{}", m.id, mode.name);
            assert!(
                mode.pixel_aspect.horizontal > 0 && mode.pixel_aspect.vertical > 0,
                "{}/{}: zero pixel-aspect component",
                m.id,
                mode.name
            );
        }
    }
}

/// Where a mode has a cell grid, the paper divides evenly into cells and
/// the grid's components are nonzero.
#[test]
fn cell_grids_divide_paper_evenly() {
    for m in machines() {
        for mode in m.modes {
            let Some(cell) = mode.cell else { continue };
            assert!(cell.width > 0 && cell.height > 0 && cell.free_colours > 0);
            assert_eq!(
                mode.paper_width % u16::from(cell.width),
                0,
                "{}/{}: paper width {} not divisible by cell width {}",
                m.id,
                mode.name,
                mode.paper_width,
                cell.width
            );
            assert_eq!(
                mode.paper_height % u16::from(cell.height),
                0,
                "{}/{}: paper height {} not divisible by cell height {}",
                m.id,
                mode.name,
                mode.paper_height,
                cell.height
            );
        }
    }
}

/// Cell-constrained modes carry a cell grid; planar modes carry a plane
/// budget instead (and `planes()` reports it).
#[test]
fn constraint_shape_matches_mode_fields() {
    for m in machines() {
        for mode in m.modes {
            match mode.constraint {
                ConstraintRule::Planar { max_planes } => {
                    assert!(mode.cell.is_none(), "{}/{}", m.id, mode.name);
                    assert!(max_planes > 0, "{}/{}", m.id, mode.name);
                    assert_eq!(mode.planes(), Some(max_planes));
                }
                _ => {
                    assert!(mode.cell.is_some(), "{}/{}", m.id, mode.name);
                    assert_eq!(mode.planes(), None);
                }
            }
        }
    }
}

/// Fixed-palette machines: every interpretation carries the machine's full
/// colour count (all interpretations of one machine agree on it), with a
/// unique name and a nonempty provenance citation. The pinned default
/// resolves to a real interpretation. Gamut machines have no default.
#[test]
fn fixed_palettes_complete_and_defaults_pinned() {
    for m in machines() {
        match m.palette {
            PaletteModel::Fixed(palettes) => {
                assert!(!palettes.is_empty(), "{}: no interpretations", m.id);
                let count = palettes[0].colours.len();
                for (i, p) in palettes.iter().enumerate() {
                    assert_eq!(
                        p.colours.len(),
                        count,
                        "{}/{}: colour count differs from the machine's",
                        m.id,
                        p.name
                    );
                    assert!(!p.source.is_empty(), "{}/{}: no provenance", m.id, p.name);
                    assert!(
                        palettes
                            .iter()
                            .skip(i + 1)
                            .all(|other| other.name != p.name),
                        "{}: duplicate interpretation name {:?}",
                        m.id,
                        p.name
                    );
                }
                let default = m
                    .default_interpretation
                    .unwrap_or_else(|| panic!("{}: fixed palette without pinned default", m.id));
                assert!(
                    m.interpretation(default).is_some(),
                    "{}: default interpretation {:?} does not resolve",
                    m.id,
                    default
                );
                assert!(m.default_palette().is_some());
            }
            PaletteModel::Gamut { bits_per_gun } => {
                assert!(bits_per_gun > 0, "{}: zero-bit gamut", m.id);
                assert!(
                    m.default_interpretation.is_none(),
                    "{}: gamut machines have no named interpretations",
                    m.id
                );
            }
            // PaletteModel is non_exhaustive; a new variant must come with
            // its own consistency checks here.
            _ => panic!("{}: unchecked palette model variant", m.id),
        }
    }
}
