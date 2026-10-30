pub(crate) const ICON_SIZE: u32 = 32;

/// Draw in logical 16px coordinates, supersampled for a crisp Windows tray icon.
pub(crate) fn render_quota_icon(levels: [Option<u8>; 4]) -> Vec<u8> {
    use std::f64::consts::{FRAC_PI_2, TAU};
    let radius = 6.3;
    let half_stroke = 0.65;
    let gap = 10.0_f64.to_radians();
    let sweep = FRAC_PI_2 - gap * 2.0;
    let sample = |x: f64, y: f64| -> [u8; 4] {
        let dx = x - 8.0;
        let dy = y - 8.0;
        if dx.abs() + dy.abs() <= 3.1 {
            return [153, 159, 169, 180];
        }
        let distance = dx.hypot(dy);
        let angle = (dy.atan2(dx) + FRAC_PI_2).rem_euclid(TAU);
        // Clockwise quadrants from the top: Codex, WorkBuddy, TRAE, AtomCode.
        let index = (angle / FRAC_PI_2).floor() as usize;
        let start = index as f64 * FRAC_PI_2 + gap;
        let arc_distance = |length: f64| {
            if angle >= start && angle <= start + length {
                (distance - radius).abs()
            } else {
                let endpoint_distance =
                    |a: f64| (dx - radius * a.sin()).hypot(dy + radius * a.cos());
                endpoint_distance(start).min(endpoint_distance(start + length))
            }
        };
        if arc_distance(sweep) > half_stroke {
            return [0; 4];
        }
        let Some(percent) = levels[index] else {
            return [140, 147, 158, 45];
        };
        let percent = percent.min(100);
        if percent > 0 && arc_distance(sweep * percent as f64 / 100.0) <= half_stroke {
            match percent {
                1..=10 => [220, 70, 65, 255],
                11..=30 => [205, 141, 28, 255],
                _ => [112, 118, 126, 255],
            }
        } else {
            [140, 147, 158, 85]
        }
    };
    let mut pixels = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let mut sum = [0u32; 4];
            for sy in 0..4 {
                for sx in 0..4 {
                    let color = sample(
                        (x as f64 + (sx as f64 + 0.5) / 4.0) / 2.0,
                        (y as f64 + (sy as f64 + 0.5) / 4.0) / 2.0,
                    );
                    for channel in 0..3 {
                        sum[channel] += color[channel] as u32 * color[3] as u32;
                    }
                    sum[3] += color[3] as u32;
                }
            }
            for channel in 0..3 {
                pixels.push(if sum[3] == 0 {
                    0
                } else {
                    (sum[channel] / sum[3]) as u8
                });
            }
            pixels.push((sum[3] / 16) as u8);
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_segmented_ring_around_a_center_diamond() {
        let pixels = render_quota_icon([Some(100); 4]);
        assert_eq!(pixels.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
        assert_eq!(&pixels[..4], &[0, 0, 0, 0]);
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] == 255));
        let alpha = |x: usize, y: usize| pixels[(y * ICON_SIZE as usize + x) * 4 + 3];
        assert!(alpha(16, 16) > 0, "center diamond must be visible");
        assert_eq!(alpha(16, 8), 0, "leave space between diamond and ring");
        assert!(alpha(25, 6) > 0, "outer circular progress must be visible");
    }

    #[test]
    fn each_provider_changes_its_own_part_of_the_same_icon() {
        let empty = render_quota_icon([Some(0); 4]);
        for provider in 0..4 {
            let mut levels = [Some(0); 4];
            levels[provider] = Some(100);
            let changed = render_quota_icon(levels);
            let differences: Vec<_> = changed
                .chunks_exact(4)
                .zip(empty.chunks_exact(4))
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(index, _)| index)
                .collect();
            assert!(
                !differences.is_empty(),
                "provider {provider} must be visible"
            );
            assert!(
                differences.iter().all(|index| {
                    let x = index % ICON_SIZE as usize;
                    let y = index / ICON_SIZE as usize;
                    match provider {
                        0 => x >= 16 && y < 16,
                        1 => x >= 16 && y >= 16,
                        2 => x < 16 && y >= 16,
                        _ => x < 16 && y < 16,
                    }
                }),
                "a provider must not overwrite the others"
            );
        }
    }

    #[test]
    fn distinguishes_unknown_empty_low_and_full_quota() {
        let icons: Vec<_> = [None, Some(0), Some(10), Some(25), Some(50), Some(100)]
            .map(|level| render_quota_icon([level; 4]))
            .into_iter()
            .collect();
        for (index, icon) in icons.iter().enumerate() {
            for other in &icons[index + 1..] {
                assert!(icon != other, "different quota states must look different");
            }
        }
        assert_eq!(
            render_quota_icon([Some(200); 4]),
            render_quota_icon([Some(100); 4])
        );
    }
}
