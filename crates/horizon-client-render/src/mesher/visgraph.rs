//! Section face connectivity using 16-bit rows. Closed cavities cannot connect
//! exterior faces, so only components reachable from the boundary are visited.

pub const ALL_VISIBLE: u64 = (1 << 36) - 1;

#[inline]
pub(super) fn resolve(opaque: &[bool; 4096], opaque_count: usize, stack: &mut Vec<usize>) -> u64 {
    stack.clear();
    if opaque_count == 0 {
        return ALL_VISIBLE;
    }
    if opaque_count == 4096 {
        return 0;
    }
    resolve_mixed(opaque, stack)
}

fn resolve_mixed(opaque: &[bool; 4096], stack: &mut Vec<usize>) -> u64 {
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
#[path = "tests/visgraph.rs"]
mod tests;
