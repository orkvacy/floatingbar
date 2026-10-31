//! Window geometry is calculated in physical pixels, including mixed-DPI monitors.
#[derive(Clone, Copy)]
pub struct WorkArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub expand_up: bool,
}

/// Preserve the compact pill's center while allocating expansion space on-screen.
pub fn calculate(anchor: (f64, f64), work: WorkArea, scale: f64) -> Placement {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let width = (440.0 * scale)
        .round()
        .max(1.0)
        .min(work.width.max(1) as f64);
    let work_height = work.height.max(1) as f64;
    let inset = (23.0 * scale).min(work_height / 2.0);
    let center_y = anchor
        .1
        .clamp(work.y as f64 + inset, work.y as f64 + work_height - inset);
    let expand_up = center_y >= work.y as f64 + work_height / 2.0;
    let edge = if expand_up {
        center_y + inset
    } else {
        center_y - inset
    };
    let available = if expand_up {
        edge - work.y as f64
    } else {
        work.y as f64 + work_height - edge
    };
    let height = (640.0 * scale).min(available).round().max(1.0) as u32;
    Placement {
        x: (anchor.0 - width / 2.0)
            .clamp(
                work.x as f64,
                work.x as f64 + work.width.max(1) as f64 - width,
            )
            .round() as i32,
        y: if expand_up {
            (edge - height as f64).round() as i32
        } else {
            edge.round() as i32
        },
        width: width as u32,
        height,
        expand_up,
    }
}

pub fn compact_center(
    origin: (i32, i32),
    size: (u32, u32),
    expand_up: bool,
    scale: f64,
) -> (f64, f64) {
    (
        origin.0 as f64 + size.0 as f64 / 2.0,
        origin.1 as f64
            + if expand_up {
                size.1 as f64 - 23.0 * scale
            } else {
                23.0 * scale
            },
    )
}

/// Header grab coordinates are in CSS pixels; the global cursor is physical.
/// Moving the full transparent window must not use the OS caption-drag clamp.
pub fn drag_origin(cursor: (i32, i32), grab: (f64, f64), scale: f64) -> (i32, i32) {
    (
        (cursor.0 as f64 - grab.0 * scale).round() as i32,
        (cursor.1 as f64 - grab.1 * scale).round() as i32,
    )
}

pub fn drop_anchor(
    cursor: (i32, i32),
    grab: (f64, f64),
    header_center: (f64, f64),
    scale: f64,
) -> (f64, f64) {
    (
        cursor.0 as f64 + (header_center.0 - grab.0) * scale,
        cursor.1 as f64 + (header_center.1 - grab.1) * scale,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn work() -> WorkArea {
        WorkArea {
            x: 0,
            y: 0,
            width: 1920,
            height: 1040,
        }
    }
    fn assert_inside(p: Placement, work: WorkArea) {
        assert!(p.x >= work.x && p.y >= work.y);
        assert!(p.x as i64 + p.width as i64 <= work.x as i64 + work.width as i64);
        assert!(p.y as i64 + p.height as i64 <= work.y as i64 + work.height as i64);
    }
    #[test]
    fn bottom_expands_up_without_overlapping_taskbar() {
        let p = calculate((960.0, 1020.0), work(), 1.0);
        assert!(p.expand_up);
        assert_inside(p, work());
        assert_eq!(p.y + p.height as i32, 1040);
    }
    #[test]
    fn top_expands_down_and_preserves_compact_position() {
        let p = calculate((700.0, 100.0), work(), 1.0);
        assert!(!p.expand_up);
        assert_inside(p, work());
        let center = compact_center((p.x, p.y), (p.width, p.height), p.expand_up, 1.0);
        assert_eq!(center, (700.0, 100.0));
    }
    #[test]
    fn middle_resizes_available_space_instead_of_clipping() {
        let p = calculate((960.0, 500.0), work(), 1.0);
        assert_inside(p, work());
        assert!(p.height < 640);
    }
    #[test]
    fn negative_monitor_coordinates_and_high_dpi_stay_on_screen() {
        let work = WorkArea {
            x: -1920,
            y: -200,
            width: 1920,
            height: 1040,
        };
        let p = calculate((-30.0, 810.0), work, 1.5);
        assert_inside(p, work);
        assert!(p.expand_up);
        assert_eq!(p.width, 660);
    }
    #[test]
    fn offscreen_anchor_is_clamped_on_all_edges() {
        for x in [-9000.0, 9000.0] {
            for y in [-9000.0, 9000.0] {
                assert_inside(calculate((x, y), work(), 1.0), work());
            }
        }
    }
    #[test]
    fn small_work_area_reduces_window_instead_of_overflowing() {
        let work = WorkArea {
            x: 10,
            y: 20,
            width: 320,
            height: 240,
        };
        assert_inside(calculate((100.0, 200.0), work, 2.0), work);
    }
    #[test]
    fn repeated_bottom_placement_does_not_drift() {
        let first = calculate((1000.0, 900.0), work(), 1.25);
        let anchor = compact_center(
            (first.x, first.y),
            (first.width, first.height),
            first.expand_up,
            1.25,
        );
        let next = calculate(anchor, work(), 1.25);
        assert_eq!(
            (first.x, first.y, first.height),
            (next.x, next.y, next.height)
        );
    }

    #[test]
    fn expanded_bottom_header_can_be_dragged_all_the_way_to_top() {
        // An upward-opening window has its header hundreds of pixels above
        // the compact anchor. The drop must use that header, not its old bottom.
        let cursor = (550, 55);
        let grab = (110.0, 390.0);
        let header = (220.0, 400.0);
        let origin = drag_origin(cursor, grab, 1.0);
        assert!(
            origin.1 < 0,
            "temporary window movement may cross the monitor edge"
        );
        let anchor = drop_anchor(cursor, grab, header, 1.0);
        assert_eq!(anchor, (660.0, 65.0));
        let placed = calculate(anchor, work(), 1.0);
        assert!(!placed.expand_up);
        assert_inside(placed, work());
        assert_eq!(
            compact_center(
                (placed.x, placed.y),
                (placed.width, placed.height),
                false,
                1.0
            ),
            anchor
        );
    }

    #[test]
    fn top_to_bottom_drag_follows_cursor_without_horizontal_recentering() {
        let anchor = drop_anchor((1500, 1000), (110.0, 35.0), (220.0, 40.0), 1.0);
        let placed = calculate(anchor, work(), 1.0);
        assert!(placed.expand_up);
        assert_eq!(placed.x + placed.width as i32 / 2, 1610);
        assert_inside(placed, work());
    }

    #[test]
    fn drag_grip_offsets_convert_to_physical_pixels_at_high_dpi() {
        assert_eq!(drag_origin((900, 600), (100.0, 200.0), 1.5), (750, 300));
        assert_eq!(
            drop_anchor((900, 600), (100.0, 200.0), (120.0, 190.0), 1.5),
            (930.0, 585.0)
        );
    }
}
