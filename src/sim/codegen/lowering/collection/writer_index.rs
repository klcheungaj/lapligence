//! Which writers may share storage with a set of writes.
//!
//! The single-writer rules compare writers pairwise, but two writers conflict
//! only when they write the same cells or bits. The index holds every write
//! under its storage with the interval it covers, so a query meets the
//! overlapping writers instead of every writer of the design.

use super::*;

/// The storage two dependencies must have in common to overlap
/// ([`Codegen::same_storage`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum StorageKey<'a> {
    Name(&'a str),
    /// One element of a fixed array, by its bits.
    Cell(usize, u64),
    /// The elements of a fixed array that are written alone, by their index.
    Cells(usize),
    /// The rows and the whole of a fixed array, by their cells.
    Rows(usize),
    Container(usize),
    Object(usize),
    /// Storage that only an equal dependency overlaps.
    Other,
}

/// The half-open interval of a write that covers all of its storage.
const WHOLE: (u64, u64) = (0, u64::MAX);

/// The writers of one storage.
#[derive(Default)]
struct Bucket {
    whole: Vec<usize>,
    /// `(start, end, writer)` of the writes of a part, by start.
    parts: Vec<(u64, u64, usize)>,
    /// The greatest end of the parts up to each one.
    greatest_end: Vec<u64>,
}

impl Bucket {
    fn insert(&mut self, interval: (u64, u64), writer: usize) {
        if interval == WHOLE {
            self.whole.push(writer);
        } else {
            self.parts.push((interval.0, interval.1, writer));
        }
    }

    fn finish(&mut self) {
        self.parts.sort_unstable();
        let mut greatest = 0;
        self.greatest_end = self
            .parts
            .iter()
            .map(|part| {
                greatest = greatest.max(part.1);
                greatest
            })
            .collect();
    }

    fn overlapping(&self, (start, end): (u64, u64), writers: &mut Vec<usize>) {
        writers.extend(&self.whole);
        let after = self.parts.partition_point(|part| part.0 < end);
        for (part, greatest) in self.parts[..after].iter().zip(&self.greatest_end).rev() {
            if *greatest <= start {
                break;
            }
            if part.1 > start {
                writers.push(part.2);
            }
        }
    }
}

/// The writes of a list of writers by storage. A writer is named by its
/// position in the list.
pub(super) struct WriterIndex<'a> {
    buckets: HashMap<StorageKey<'a>, Bucket>,
}

impl<'a> WriterIndex<'a> {
    pub(super) fn new(
        codegen: &Codegen<'_>,
        writers: impl IntoIterator<Item = &'a HashSet<IrDependency>>,
    ) -> Self {
        let mut buckets = HashMap::<_, Bucket>::new();
        for (writer, writes) in writers.into_iter().enumerate() {
            for write in writes {
                let (key, interval) = codegen.storage_interval(write);
                buckets.entry(key).or_default().insert(interval, writer);
                if let StorageKey::Cell(array, index) = key {
                    buckets
                        .entry(StorageKey::Cells(array))
                        .or_default()
                        .insert((index, index.saturating_add(1)), writer);
                }
            }
        }
        for bucket in buckets.values_mut() {
            bucket.finish();
        }
        Self { buckets }
    }

    /// The writers that may overlap one of `writes`, ascending: every writer
    /// with a write of the [`Codegen::same_storage`] is among them.
    pub(super) fn overlapping<'w>(
        &self,
        codegen: &Codegen<'_>,
        writes: impl IntoIterator<Item = &'w IrDependency>,
    ) -> Vec<usize> {
        let mut writers = Vec::new();
        for write in writes {
            let (key, interval) = codegen.storage_interval(write);
            let mut collect = |key: StorageKey<'_>, interval| {
                if let Some(bucket) = self.buckets.get(&key) {
                    bucket.overlapping(interval, &mut writers);
                }
            };
            collect(key, interval);
            match key {
                StorageKey::Cell(array, index) => {
                    collect(StorageKey::Rows(array), (index, index.saturating_add(1)))
                }
                StorageKey::Rows(array) => collect(StorageKey::Cells(array), interval),
                _ => {}
            }
        }
        writers.sort_unstable();
        writers.dedup();
        writers
    }
}

impl Codegen<'_> {
    /// The storage `dependency` names and the interval of it that it covers.
    fn storage_interval<'a>(&self, dependency: &'a IrDependency) -> (StorageKey<'a>, (u64, u64)) {
        match dependency {
            IrDependency::PackedRange {
                storage,
                lsb,
                width,
            } => {
                let (key, interval) = self.storage_interval(storage);
                if interval != WHOLE {
                    return (key, WHOLE);
                }
                let start = u64::from(*lsb);
                (key, (start, start.saturating_add(u64::from(*width).max(1))))
            }
            IrDependency::Scalar(name) | IrDependency::Real(name) => {
                (StorageKey::Name(name), WHOLE)
            }
            IrDependency::ArrayElement { array, index } => {
                (StorageKey::Cell(*array, *index), WHOLE)
            }
            IrDependency::ArrayContents(array) => (StorageKey::Rows(*array), WHOLE),
            IrDependency::ContainerContents(container)
            | IrDependency::ContainerShape(container) => (StorageKey::Container(*container), WHOLE),
            IrDependency::Object(object) => (StorageKey::Object(*object), WHOLE),
            _ => (StorageKey::Other, WHOLE),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::dependencies::SAME_STORAGE_COMPARISONS;
    use super::*;

    fn database(source: &str) -> Db {
        let output = crate::core::compile::compile_sources_checked(
            &[crate::core::compile::OwnedSource::compilation_unit(
                "writers.sv",
                source,
            )],
            &crate::core::compile::CompileOpts {
                top: Some("tb".to_owned()),
                ..Default::default()
            },
        )
        .unwrap();
        Db::from_slang(&output.snapshot).unwrap()
    }

    fn range(storage: IrDependency, lsb: u32, width: u32) -> IrDependency {
        IrDependency::PackedRange {
            storage: Box::new(storage),
            lsb,
            width,
        }
    }

    #[test]
    fn every_pair_with_common_storage_is_a_candidate() {
        let db = database(
            "module tb; logic [31:0] v; real r; logic [7:0] a[6], b[4]; \
             initial begin v = 1; r = v; a[v] = 1; b[v] = a[v]; end endmodule",
        );
        let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
        let mut cg = Codegen::new(&semantic);
        cg.collect_design().unwrap();
        let signal = |name: &str| {
            db.nodes()
                .iter()
                .enumerate()
                .filter(|(_, node)| node.name == name)
                .find_map(|(index, _)| cg.signal_of(NodeId::from_index(index)))
                .map(|signal| cg.signal_dependency(signal))
                .expect("signal")
        };
        let (v, r) = (signal("v"), signal("r"));
        let element = |array, index| IrDependency::ArrayElement { array, index };
        let mut dependencies = vec![
            range(v.clone(), 0, 8),
            range(v.clone(), 4, 8),
            range(v.clone(), 12, 20),
            range(range(v.clone(), 12, 20), 0, 4),
            v,
            r,
            IrDependency::ContainerContents(0),
            IrDependency::ContainerShape(0),
            IrDependency::ContainerShape(1),
            IrDependency::Object(0),
            IrDependency::NativeValue(0),
        ];
        for array in 0..2 {
            dependencies.push(IrDependency::ArrayContents(array));
            dependencies.extend(cg.array_row_write(array, 0, 2));
            dependencies.extend(cg.array_row_write(array, 1, 3));
            for index in 0..4 {
                dependencies.push(element(array, index));
                dependencies.push(range(element(array, index), 0, 4));
                dependencies.push(range(element(array, index), 4, 4));
            }
        }
        let writes = dependencies
            .iter()
            .map(|dependency| HashSet::from([dependency.clone()]))
            .collect::<Vec<_>>();
        let index = WriterIndex::new(&cg, &writes);
        let mut conflicts = 0;
        let mut candidates = 0;
        for dependency in &dependencies {
            let found = index.overlapping(&cg, [dependency]);
            assert!(found.is_sorted_by(|left, right| left < right));
            candidates += found.len();
            for (writer, other) in dependencies.iter().enumerate() {
                if cg.same_storage(dependency, other) {
                    conflicts += 1;
                    assert!(found.contains(&writer), "{dependency:?} and {other:?}");
                }
            }
        }
        // Only a nested range and storage that is never the same are
        // candidates without a conflict.
        assert!(candidates - conflicts <= 12, "{candidates} for {conflicts}");
        assert!(index.overlapping(&cg, [&range(element(1, 3), 0, 4)]).len() < 6);
    }

    /// The single-writer rules once compared every pair of writers.
    #[test]
    fn validation_compares_the_writers_of_one_storage() {
        let comparisons = |instances: usize| {
            let db = database(&format!(
                "module m(input logic clk, input logic [15:0] d, output logic [15:0] o); \
                 logic [15:0] s, q; logic [7:0] mem[4]; \
                 assign o = q ^ d; always_comb s = d + q; \
                 always_ff @(posedge clk) begin q <= s; mem[d[1:0]] <= s[7:0]; end endmodule \
                 module tb; logic clk; logic [15:0] d; logic [15:0] o[{instances}]; \
                 logic [{instances}-1:0] bits; \
                 for (genvar g = 0; g < {instances}; g++) begin : gen \
                 m u(.clk(clk), .d(d), .o(o[g])); assign bits[g] = ^o[g]; end \
                 initial begin clk = 0; d = 0; end endmodule"
            ));
            let semantic = crate::sim::semantic::SemanticModel::from_db(&db);
            let mut cg = Codegen::new(&semantic);
            cg.collect_design().unwrap();
            cg.bind_reference_ports().unwrap();
            cg.collect_timescales();
            cg.build_net_groups().unwrap();
            SAME_STORAGE_COMPARISONS.with(|work| work.set(0));
            cg.validate_process_semantics().unwrap();
            SAME_STORAGE_COMPARISONS.with(std::cell::Cell::get)
        };
        // No two writers of this design name common storage; 320 writers
        // make about 50000 pairs.
        let compared = comparisons(80);
        assert!(compared <= 320, "{compared}");
    }
}
