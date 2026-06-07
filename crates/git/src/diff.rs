use std::ops::Range;
use sum_tree::SumTree;
use text::{Anchor, BufferSnapshot, OffsetRangeExt, Point};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffHunkStatus {
    Added,
    Modified,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk<T> {
    pub buffer_range: Range<T>,
    pub diff_base_byte_range: Range<usize>,
}

impl DiffHunk<u32> {
    pub fn status(&self) -> DiffHunkStatus {
        if self.diff_base_byte_range.is_empty() {
            DiffHunkStatus::Added
        } else if self.buffer_range.is_empty() {
            DiffHunkStatus::Removed
        } else {
            DiffHunkStatus::Modified
        }
    }
}

impl sum_tree::Item for DiffHunk<Anchor> {
    type Summary = DiffHunkSummary;

    fn summary(&self) -> Self::Summary {
        DiffHunkSummary {
            buffer_range: self.buffer_range.clone(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct DiffHunkSummary {
    buffer_range: Range<Anchor>,
}

impl sum_tree::Summary for DiffHunkSummary {
    type Context = text::BufferSnapshot;

    fn add_summary(&mut self, other: &Self, buffer: &Self::Context) {
        self.buffer_range.start = self
            .buffer_range
            .start
            .min(&other.buffer_range.start, buffer);
        self.buffer_range.end = self.buffer_range.end.max(&other.buffer_range.end, buffer);
    }
}

#[derive(Clone)]
pub struct BufferDiff {
    last_buffer_version: Option<clock::Global>,
    tree: SumTree<DiffHunk<Anchor>>,
}

impl BufferDiff {
    pub fn new() -> BufferDiff {
        BufferDiff {
            last_buffer_version: None,
            tree: SumTree::new(),
        }
    }

    pub fn hunks_in_row_range<'a>(
        &'a self,
        range: Range<u32>,
        buffer: &'a BufferSnapshot,
        reversed: bool,
    ) -> impl 'a + Iterator<Item = DiffHunk<u32>> {
        let start = buffer.anchor_before(Point::new(range.start, 0));
        let end = buffer.anchor_after(Point::new(range.end, 0));
        self.hunks_intersecting_range(start..end, buffer, reversed)
    }

    pub fn hunks_intersecting_range<'a>(
        &'a self,
        range: Range<Anchor>,
        buffer: &'a BufferSnapshot,
        reversed: bool,
    ) -> impl 'a + Iterator<Item = DiffHunk<u32>> {
        let mut cursor = self.tree.filter::<_, DiffHunkSummary>(move |summary| {
            let before_start = summary.buffer_range.end.cmp(&range.start, buffer).is_lt();
            let after_end = summary.buffer_range.start.cmp(&range.end, buffer).is_gt();
            !before_start && !after_end
        });

        std::iter::from_fn(move || {
            if reversed {
                cursor.prev(buffer);
            } else {
                cursor.next(buffer);
            }

            let hunk = cursor.item()?;

            let range = hunk.buffer_range.to_point(buffer);
            let end_row = if range.end.column > 0 {
                range.end.row + 1
            } else {
                range.end.row
            };

            Some(DiffHunk {
                buffer_range: range.start.row..end_row,
                diff_base_byte_range: hunk.diff_base_byte_range.clone(),
            })
        })
    }

    pub fn clear(&mut self, buffer: &text::BufferSnapshot) {
        self.last_buffer_version = Some(buffer.version().clone());
        self.tree = SumTree::new();
    }

    pub fn needs_update(&self, buffer: &text::BufferSnapshot) -> bool {
        match &self.last_buffer_version {
            Some(last) => buffer.version().changed_since(last),
            None => true,
        }
    }

    pub async fn update(&mut self, diff_base: &str, buffer: &text::BufferSnapshot) {
        let mut tree = SumTree::new();

        let buffer_text = buffer.as_rope().to_string();
        for hunk in Self::diff(diff_base, &buffer_text, buffer) {
            tree.push(hunk, buffer);
        }

        self.tree = tree;
        self.last_buffer_version = Some(buffer.version().clone());
    }

    #[cfg(test)]
    fn hunks<'a>(&'a self, text: &'a BufferSnapshot) -> impl 'a + Iterator<Item = DiffHunk<u32>> {
        let start = text.anchor_before(Point::new(0, 0));
        let end = text.anchor_after(Point::new(u32::MAX, u32::MAX));
        self.hunks_intersecting_range(start..end, text, false)
    }

    fn diff(
        diff_base: &str,
        current: &str,
        buffer: &text::BufferSnapshot,
    ) -> Vec<DiffHunk<Anchor>> {
        let diff_base_line_offsets = line_start_offsets(diff_base);
        similar::TextDiff::from_lines(diff_base, current)
            .grouped_ops(0)
            .into_iter()
            .filter_map(|group| {
                let mut old_range: Option<Range<usize>> = None;
                let mut buffer_row_range: Option<Range<u32>> = None;

                for operation in group {
                    let old_operation_range = operation.old_range();
                    let new_operation_range = operation.new_range();

                    if !old_operation_range.is_empty() {
                        old_range = Some(match old_range {
                            Some(old_range) => {
                                old_range.start.min(old_operation_range.start)
                                    ..old_range.end.max(old_operation_range.end)
                            }
                            None => old_operation_range,
                        });
                    }

                    if !new_operation_range.is_empty() {
                        let operation_buffer_range =
                            new_operation_range.start as u32..new_operation_range.end as u32;
                        buffer_row_range = Some(match buffer_row_range {
                            Some(buffer_row_range) => {
                                buffer_row_range.start.min(operation_buffer_range.start)
                                    ..buffer_row_range.end.max(operation_buffer_range.end)
                            }
                            None => operation_buffer_range,
                        });
                    }
                }

                let old_range = old_range.unwrap_or(0..0);
                let buffer_row_range = buffer_row_range
                    .unwrap_or_else(|| old_range.start as u32..old_range.start as u32);

                let diff_base_byte_range = if old_range.is_empty() {
                    0..0
                } else {
                    diff_base_line_offsets[old_range.start]..diff_base_line_offsets[old_range.end]
                };

                let start = Point::new(buffer_row_range.start, 0);
                let end = Point::new(buffer_row_range.end, 0);
                Some(DiffHunk {
                    buffer_range: buffer.anchor_before(start)..buffer.anchor_before(end),
                    diff_base_byte_range,
                })
            })
            .collect()
    }
}

fn line_start_offsets(text: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            offsets.push(index + 1);
        }
    }
    if offsets.last().copied() != Some(text.len()) {
        offsets.push(text.len());
    }
    offsets
}

/// Range (crossing new lines), old, new
#[cfg(any(test, feature = "test-support"))]
#[track_caller]
pub fn assert_hunks<Iter>(
    diff_hunks: Iter,
    buffer: &BufferSnapshot,
    diff_base: &str,
    expected_hunks: &[(Range<u32>, &str, &str)],
) where
    Iter: Iterator<Item = DiffHunk<u32>>,
{
    let actual_hunks = diff_hunks
        .map(|hunk| {
            (
                hunk.buffer_range.clone(),
                &diff_base[hunk.diff_base_byte_range],
                buffer
                    .text_for_range(
                        Point::new(hunk.buffer_range.start, 0)
                            ..Point::new(hunk.buffer_range.end, 0),
                    )
                    .collect::<String>(),
            )
        })
        .collect::<Vec<_>>();

    let expected_hunks: Vec<_> = expected_hunks
        .iter()
        .map(|(r, s, h)| (r.clone(), *s, h.to_string()))
        .collect();

    assert_eq!(actual_hunks, expected_hunks);
}

#[cfg(test)]
mod tests {
    use super::*;
    use text::Buffer;
    use unindent::Unindent as _;

    #[test]
    fn test_buffer_diff_simple() {
        let diff_base = "
            one
            two
            three
        "
        .unindent();

        let buffer_text = "
            one
            HELLO
            three
        "
        .unindent();

        let mut buffer = Buffer::new(0, 0, buffer_text);
        let mut diff = BufferDiff::new();
        smol::block_on(diff.update(&diff_base, &buffer));
        assert_hunks(
            diff.hunks(&buffer),
            &buffer,
            &diff_base,
            &[(1..2, "two\n", "HELLO\n")],
        );

        buffer.edit([(0..0, "point five\n")]);
        smol::block_on(diff.update(&diff_base, &buffer));
        assert_hunks(
            diff.hunks(&buffer),
            &buffer,
            &diff_base,
            &[(0..1, "", "point five\n"), (2..3, "two\n", "HELLO\n")],
        );

        diff.clear(&buffer);
        assert_hunks(diff.hunks(&buffer), &buffer, &diff_base, &[]);
    }

    #[test]
    fn test_buffer_diff_range() {
        let diff_base = "
            one
            two
            three
            four
            five
            six
            seven
            eight
            nine
            ten
        "
        .unindent();

        let buffer_text = "
            A
            one
            B
            two
            C
            three
            HELLO
            four
            five
            SIXTEEN
            seven
            eight
            WORLD
            nine

            ten

        "
        .unindent();

        let buffer = Buffer::new(0, 0, buffer_text);
        let mut diff = BufferDiff::new();
        smol::block_on(diff.update(&diff_base, &buffer));
        assert_eq!(diff.hunks(&buffer).count(), 8);

        assert_hunks(
            diff.hunks_in_row_range(7..12, &buffer, false),
            &buffer,
            &diff_base,
            &[
                (6..7, "", "HELLO\n"),
                (9..10, "six\n", "SIXTEEN\n"),
                (12..13, "", "WORLD\n"),
            ],
        );
    }
}
