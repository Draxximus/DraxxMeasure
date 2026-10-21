//! Plain Unicode bundle progress, independent of terminal libraries and storage.
//!
//! Positions describe effective occupancy in capture order, not historical row
//! numbers. Invalidated observations do not consume a position. Callers supply
//! committed capacity; the renderer never increments a local capture counter.
use draxx_core::{BundleCapacity, MAX_BUNDLE_MEASUREMENTS, Sequence};

pub const BUNDLE_LAYERS: [usize; 5] = [8, 9, 8, 9, 8];
const _: () = assert!(
    BUNDLE_LAYERS[0] + BUNDLE_LAYERS[1] + BUNDLE_LAYERS[2] + BUNDLE_LAYERS[3] + BUNDLE_LAYERS[4]
        == MAX_BUNDLE_MEASUREMENTS
);

/// Render exactly 42 staggered positions, filled from left to right, top to bottom.
/// The caller owns prompts, publication, and terminal updates. No ANSI escapes or
/// graphical dependencies are required; both terminal adapters reuse this text.
pub fn render_bundle(sequence: Sequence, capacity: BundleCapacity) -> String {
    let mut text = format!("Bundle #{sequence} — {}/42 measured\n", capacity.active());
    let mut position = 0;
    for layer in BUNDLE_LAYERS {
        text.push('\n');
        text.push_str(if layer == 8 { "   " } else { " " });
        for column in 0..layer {
            if column != 0 {
                text.push_str("   ");
            }
            text.push(if position < capacity.active() {
                '●'
            } else {
                '○'
            });
            position += 1;
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use draxx_application::{InspectionService, PartSetup};
    use draxx_core::*;
    use draxx_storage::{InspectionStore, SqliteStore};
    use std::path::Path;

    fn render(count: usize) -> String {
        render_bundle(
            Sequence::new(1).unwrap(),
            BundleCapacity::new(count).unwrap(),
        )
    }

    #[test]
    fn empty_bundle_matches_the_staggered_physical_arrangement() {
        assert_eq!(
            render(0),
            concat!(
                "Bundle #1 — 0/42 measured\n\n",
                "   ○   ○   ○   ○   ○   ○   ○   ○\n",
                " ○   ○   ○   ○   ○   ○   ○   ○   ○\n",
                "   ○   ○   ○   ○   ○   ○   ○   ○\n",
                " ○   ○   ○   ○   ○   ○   ○   ○   ○\n",
                "   ○   ○   ○   ○   ○   ○   ○   ○",
            )
        );
    }

    #[test]
    fn seventeen_measurements_fill_the_first_two_layers_only() {
        let text = render(17);
        assert!(text.starts_with("Bundle #1 — 17/42 measured\n\n"));
        let layers: Vec<_> = text.lines().skip(2).collect();
        assert_eq!(layers[0], "   ●   ●   ●   ●   ●   ●   ●   ●");
        assert_eq!(layers[1], " ●   ●   ●   ●   ●   ●   ●   ●   ●");
        assert!(layers[2..].iter().all(|row| !row.contains('●')));
        assert_eq!(text.matches('●').count(), 17);
        assert_eq!(text.matches('○').count(), 25);
    }

    #[test]
    fn full_bundle_has_all_forty_two_positions_filled() {
        let text = render(42);
        assert!(text.starts_with("Bundle #1 — 42/42 measured"));
        assert_eq!(text.matches('●').count(), 42);
        assert!(!text.contains('○'));
    }

    #[test]
    fn every_occupancy_preserves_layers_and_fills_exactly_one_new_position() {
        for count in 0..=42 {
            let text = render(count);
            let layers: Vec<_> = text.lines().skip(2).collect();
            assert_eq!(layers.len(), 5);
            for (row, expected) in layers.iter().zip([8, 9, 8, 9, 8]) {
                assert_eq!(row.matches(['○', '●']).count(), expected);
            }
            let positions: Vec<_> = text.chars().filter(|ch| matches!(ch, '○' | '●')).collect();
            assert_eq!(positions.len(), 42);
            assert!(positions[..count].iter().all(|ch| *ch == '●'));
            assert!(positions[count..].iter().all(|ch| *ch == '○'));
            assert!(!text.contains('\x1b'));
        }
    }

    fn setup(path: &Path) -> InspectionService<SqliteStore> {
        let mut service = InspectionService::new(SqliteStore::open(path).unwrap());
        service.start_session().unwrap();
        service
            .create_part(PartSetup {
                part_number: PartNumber::new("SYNTHETIC".into()).unwrap(),
                nominal: Length::parse_mm("9368.9").unwrap(),
                tolerance: Length::parse_mm("5").unwrap(),
                resolution: Resolution::new(Length::parse_mm("0.1").unwrap()).unwrap(),
                policy: None,
            })
            .unwrap();
        service.start_bundle().unwrap();
        service
    }
    fn committed_view(service: &InspectionService<SqliteStore>) -> String {
        let state = service.state().unwrap().unwrap();
        render_bundle(
            state.bundle.unwrap().sequence(),
            BundleCapacity::new(state.active_count).unwrap(),
        )
    }

    #[test]
    fn undo_and_resume_reconstruct_effective_occupancy_from_sqlite() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("inspection.sqlite3");
        let mut service = setup(&path);
        for _ in 0..17 {
            service.record("=9368".into()).unwrap();
        }
        let session = service.state().unwrap().unwrap().session.id();
        assert_eq!(committed_view(&service), render(17));
        let target = service.target(None).unwrap();
        service
            .correct(target, None, "Synthetic undo".into())
            .unwrap();
        assert_eq!(committed_view(&service), render(16));
        drop(service);
        let mut recovered = InspectionService::new(SqliteStore::open(path).unwrap());
        recovered.resume(session).unwrap();
        assert_eq!(committed_view(&recovered), render(16));
        let snapshot = recovered.current_snapshot().unwrap();
        assert_eq!(snapshot.bundles[0].measurements.len(), 17);
        assert_eq!(
            snapshot.bundles[0].measurements[16].status(),
            MeasurementStatus::Invalidated
        );
        recovered.record("=9369".into()).unwrap();
        assert_eq!(committed_view(&recovered), render(17));
    }

    #[test]
    fn failed_capture_and_capacity_rejection_do_not_fill_positions_or_close_bundle() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("inspection.sqlite3");
        let mut service = setup(&path);
        assert!(service.record("bad".into()).is_err());
        assert_eq!(committed_view(&service), render(0));
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch("CREATE TRIGGER synthetic_failure BEFORE INSERT ON measurements BEGIN SELECT RAISE(ABORT,'synthetic write failure'); END;").unwrap();
        assert!(service.record("=9368".into()).is_err());
        assert_eq!(committed_view(&service), render(0));
        connection
            .execute_batch("DROP TRIGGER synthetic_failure;")
            .unwrap();
        service.retry().unwrap();
        for _ in 1..42 {
            service.record("=9368".into()).unwrap();
        }
        assert_eq!(committed_view(&service), render(42));
        assert!(service.record("=9369".into()).is_err());
        assert_eq!(committed_view(&service), render(42));
        let state = service.state().unwrap().unwrap();
        assert_eq!(state.bundle.unwrap().status(), BundleStatus::Open);
        assert_eq!(
            service
                .store()
                .recover_session(state.session.id())
                .unwrap()
                .session
                .status(),
            SessionStatus::Active
        );
    }

    #[test]
    fn capture_sequence_gaps_do_not_create_extra_physical_positions() {
        let directory = tempfile::tempdir().unwrap();
        let mut service = setup(&directory.path().join("inspection.sqlite3"));
        for _ in 0..42 {
            service.record("=9368".into()).unwrap();
        }
        let target = service.target(Some(1)).unwrap();
        service
            .correct(target, None, "Synthetic invalidation".into())
            .unwrap();
        assert_eq!(committed_view(&service), render(41));
        service.record("=9369".into()).unwrap();
        let snapshot = service.current_snapshot().unwrap();
        assert_eq!(snapshot.bundles[0].measurements.len(), 43);
        assert_eq!(committed_view(&service), render(42));
    }
}
