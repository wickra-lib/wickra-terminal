//! Conformance: every panel and source is object-safe (usable as a boxed trait
//! object in a heterogeneous collection) and reports the kind it was built for.
//! This guards the trait shapes the renderers and bindings rely on.

use terminal_core::{
    build_panel, build_source, DataSource, Panel, PanelKind, PanelSpec, RectSpec, SourceKind,
    SourceSpec,
};

const KINDS: [PanelKind; 5] = [
    PanelKind::Chart,
    PanelKind::Book,
    PanelKind::Tape,
    PanelKind::Watchlist,
    PanelKind::Footprint,
];

#[test]
fn panels_are_object_safe_and_report_their_kind() {
    let panels: Vec<Box<dyn Panel>> = KINDS
        .iter()
        .map(|&kind| {
            build_panel(&PanelSpec {
                kind,
                rect: RectSpec::new(0, 0, 100, 100),
            })
        })
        .collect();

    assert_eq!(panels.len(), KINDS.len());
    for (panel, kind) in panels.iter().zip(KINDS) {
        assert_eq!(panel.kind(), kind);
    }
}

#[test]
fn sources_are_object_safe_and_report_their_kind() {
    let synth = build_source(0, &SourceSpec::Synth { seed: 1 }).unwrap();
    let replay = build_source(
        1,
        &SourceSpec::Replay {
            dataset: "[]".to_string(),
        },
    )
    .unwrap();

    let sources: Vec<Box<dyn DataSource>> = vec![synth, replay];
    assert_eq!(sources[0].id(), 0);
    assert_eq!(sources[0].kind(), SourceKind::Synth);
    assert_eq!(sources[1].id(), 1);
    assert_eq!(sources[1].kind(), SourceKind::Replay);
}
