use std::fs;
use std::path::{Path, PathBuf};

use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use rhizome_core::backend::CodeIntelligence;
use rhizome_treesitter::TreeSitterBackend;
use tempfile::TempDir;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn copy_fixture(tempdir: &TempDir, source: &Path) -> PathBuf {
    let target = tempdir.path().join(
        source
            .file_name()
            .expect("fixture file should have a file name"),
    );
    fs::copy(source, &target).expect("copy benchmark fixture");
    target
}

fn bench_get_symbols_large_rust_fixture(c: &mut Criterion) {
    let fixture = fixture_path("large_sample.rs");

    c.bench_function("get_symbols_large_rust_fixture", |b| {
        b.iter_batched(
            || {
                let tempdir = TempDir::new().expect("create temp dir");
                let path = copy_fixture(&tempdir, &fixture);
                (tempdir, path, TreeSitterBackend::new())
            },
            |(_tempdir, path, backend)| {
                let symbols = backend
                    .get_symbols(black_box(path.as_path()))
                    .expect("extract symbols from large Rust fixture");
                black_box(symbols.len());
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, bench_get_symbols_large_rust_fixture);
criterion_main!(benches);
