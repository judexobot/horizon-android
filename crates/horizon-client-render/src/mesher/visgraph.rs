//! Section face connectivity using 16-bit rows. Closed cavities cannot connect
//! exterior faces, so only components reachable from the boundary are visited.

pub const ALL_VISIBLE: u64 = (1 << 36) - 1;

pub(super) fn resolve(opaque: &[bool; 4096], opaque_count: usize, stack: &mut Vec<usize>) -> u64 {
    stack.clear();
    if opaque_count == 0 {
        return ALL_VISIBLE;
    }
    if opaque_count == 4096 {
        return 0;
    }
    let mut open = [0u16; 256];
    for (row, cells) in open.iter_mut().zip(opaque.as_chunks::<16>().0) {
        for (x, &solid) in cells.iter().enumerate() {
            *row |= u16::from(!solid) << x;
        }
    }
    let mut result = 0;
    for start in 0..256 {
        let boundary = if !(16..240).contains(&start) || start & 15 == 0 || start & 15 == 15 {
            u16::MAX
        } else {
            0x8001
        };
        while open[start] & boundary != 0 {
            let seed = 1u16 << (open[start] & boundary).trailing_zeros();
            stack.push(start << 16 | usize::from(seed));
            let mut faces = 0u8;
            while let Some(entry) = stack.pop() {
                let row = entry >> 16;
                let mut reached = entry as u16 & open[row];
                if reached == 0 {
                    continue;
                }
                // Expand whole horizontal runs without a queue entry per voxel.
                loop {
                    let expanded = (reached | reached << 1 | reached >> 1) & open[row];
                    if expanded == reached {
                        break;
                    }
                    reached = expanded;
                }
                open[row] &= !reached;
                faces |= u8::from(row < 16)
                    | u8::from(row >= 240) << 1
                    | u8::from(row & 15 == 0) << 2
                    | u8::from(row & 15 == 15) << 3
                    | u8::from(reached & 1 != 0) << 4
                    | u8::from(reached & 0x8000 != 0) << 5;
                if faces == 0b11_1111 {
                    return ALL_VISIBLE;
                }
                if row >= 16 {
                    push(&open, stack, row - 16, reached);
                }
                if row < 240 {
                    push(&open, stack, row + 16, reached);
                }
                if row & 15 != 0 {
                    push(&open, stack, row - 1, reached);
                }
                if row & 15 != 15 {
                    push(&open, stack, row + 1, reached);
                }
            }
            for a in 0..6 {
                if faces & (1 << a) != 0 {
                    result |= u64::from(faces) << (a * 6);
                }
            }
            if result == ALL_VISIBLE {
                return result;
            }
        }
    }
    result
}

fn push(open: &[u16; 256], stack: &mut Vec<usize>, row: usize, reached: u16) {
    let next = open[row] & reached;
    if next != 0 {
        stack.push(row << 16 | usize::from(next));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The previous per-cell traversal, retained as an independent reference.
    fn reference(opaque: &[bool; 4096], count: usize, stack: &mut Vec<usize>) -> u64 {
        stack.clear();
        if count == 0 {
            return ALL_VISIBLE;
        }
        if count == 4096 {
            return 0;
        }
        let mut seen = *opaque;
        let mut result = 0;
        for start in 0..4096 {
            if seen[start] {
                continue;
            }
            seen[start] = true;
            stack.push(start);
            let mut faces = 0u8;
            while let Some(c) = stack.pop() {
                let (x, y, z) = ((c & 15) as i32, (c >> 8) as i32, ((c >> 4) & 15) as i32);
                if y == 0 {
                    faces |= 1 << 0;
                }
                if y == 15 {
                    faces |= 1 << 1;
                }
                if z == 0 {
                    faces |= 1 << 2;
                }
                if z == 15 {
                    faces |= 1 << 3;
                }
                if x == 0 {
                    faces |= 1 << 4;
                }
                if x == 15 {
                    faces |= 1 << 5;
                }
                if faces == 63 {
                    return ALL_VISIBLE;
                }
                for (dx, dy, dz) in [
                    (1, 0, 0),
                    (-1, 0, 0),
                    (0, 1, 0),
                    (0, -1, 0),
                    (0, 0, 1),
                    (0, 0, -1),
                ] {
                    let (nx, ny, nz) = (x + dx, y + dy, z + dz);
                    if !(0..16).contains(&nx) || !(0..16).contains(&ny) || !(0..16).contains(&nz) {
                        continue;
                    }
                    let n = (ny << 8 | nz << 4 | nx) as usize;
                    if !seen[n] {
                        seen[n] = true;
                        stack.push(n);
                    }
                }
            }
            for a in 0..6 {
                for b in 0..6 {
                    if faces & (1 << a) != 0 && faces & (1 << b) != 0 {
                        result |= 1 << (a * 6 + b);
                    }
                }
            }
            if result == ALL_VISIBLE {
                return result;
            }
        }
        result
    }

    fn section(mut solid: impl FnMut(usize, usize, usize) -> bool) -> [bool; 4096] {
        std::array::from_fn(|i| solid(i & 15, i >> 8, (i >> 4) & 15))
    }

    fn matches(opaque: &[bool; 4096], stack: &mut Vec<usize>) {
        let count = opaque.iter().filter(|&&s| s).count();
        assert_eq!(
            resolve(opaque, count, stack),
            reference(opaque, count, &mut Vec::new())
        );
    }

    #[test]
    fn boundary_regions_and_closed_cavities_match_per_cell_traversal() {
        let mut stack = vec![usize::MAX];
        for cells in [
            [false; 4096],
            [true; 4096],
            section(|_, y, _| y < 8),
            section(|x, y, z| (x == 8 || y == 8 || z == 8) && (x != 7 || z != 7)),
            section(|x, y, z| x != 8 || y != 8 || z == 0 || z == 15),
            section(|x, y, z| (x + y + z) % 2 == 0),
        ] {
            matches(&cells, &mut stack);
        }
        for mask in 0..64 {
            let cells = section(|x, y, z| {
                !((x == 8 && z == 8 && ((y <= 8 && mask & 1 != 0) || (y >= 8 && mask & 2 != 0)))
                    || (x == 8
                        && y == 8
                        && ((z <= 8 && mask & 4 != 0) || (z >= 8 && mask & 8 != 0)))
                    || (y == 8
                        && z == 8
                        && ((x <= 8 && mask & 16 != 0) || (x >= 8 && mask & 32 != 0))))
            });
            matches(&cells, &mut stack);
        }
    }

    #[test]
    fn random_sections_match_at_every_opacity_density() {
        let mut rng = 0x1234_5678u32;
        let mut stack = Vec::new();
        for density in [1, 8, 32, 64, 96, 128, 160, 192, 224, 248, 255] {
            for _ in 0..48 {
                let cells = section(|_, _, _| {
                    rng ^= rng << 13;
                    rng ^= rng >> 17;
                    rng ^= rng << 5;
                    (rng & 255) < density
                });
                matches(&cells, &mut stack);
            }
        }
    }

    #[test]
    #[ignore = "manual optimized benchmark; timing is not a CI pass/fail threshold"]
    fn benchmark() {
        use std::{hint::black_box, time::Instant};
        let cases = [
            ("empty", [false; 4096]),
            ("solid", [true; 4096]),
            ("one_opaque", section(|x, y, z| x == 8 && y == 8 && z == 8)),
            (
                "porous",
                section(|x, y, z| (x * 17 + y * 31 + z * 13) % 5 == 0),
            ),
            ("surface", section(|_, y, _| y < 8)),
            (
                "sealed_caves",
                section(|x, y, z| x == 0 || x == 15 || y == 0 || y == 15 || z == 0 || z == 15),
            ),
            (
                "tunnels",
                section(|x, y, z| {
                    !((7..=9).contains(&x) && (7..=9).contains(&y)
                        || (7..=9).contains(&y) && (7..=9).contains(&z))
                }),
            ),
            ("checkerboard", section(|x, y, z| (x + y + z) % 2 == 0)),
        ];
        for (name, cells) in cases {
            let count = cells.iter().filter(|&&s| s).count();
            let mut old_stack = Vec::new();
            let mut new_stack = Vec::new();
            matches(&cells, &mut new_stack);
            reference(&cells, count, &mut old_stack);
            let mut old = Vec::new();
            let mut new = Vec::new();
            let mut measure = |baseline| {
                let start = Instant::now();
                for _ in 0..2048 {
                    let result = if baseline {
                        reference(black_box(&cells), black_box(count), &mut old_stack)
                    } else {
                        resolve(black_box(&cells), black_box(count), &mut new_stack)
                    };
                    black_box(result);
                }
                start.elapsed().as_nanos() as f64 / 2048.0
            };
            for round in 0..9 {
                if round % 2 == 0 {
                    old.push(measure(true));
                    new.push(measure(false));
                } else {
                    new.push(measure(false));
                    old.push(measure(true));
                }
            }
            old.sort_by(f64::total_cmp);
            new.sort_by(f64::total_cmp);
            println!(
                "VISGRAPH_BENCH {{\"case\":\"{name}\",\"baseline_ns\":{},\"optimized_ns\":{},\"speedup\":{}}}",
                old[4],
                new[4],
                old[4] / new[4]
            );
        }
    }
}
