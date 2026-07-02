//! Filesystem statement intrinsics (`MkDir`/`RmDir`) end to end through the
//! clean stack under the interactive-dev policy (real host filesystem). `Kill`
//! is covered by the native-declare/SQLiteForExcel paths; these guard the
//! directory statements ChibiPDF and similar real-world code use.
#![cfg(target_os = "windows")]

use std::path::PathBuf;

use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig};

fn run_clean(source: &str) -> Result<(), String> {
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
        .execute_source_with_variant_snapshot_clean(source)
        .map(|_| ())
        .map_err(|d| format!("{:?}: {}", d.phase(), d.message()))
}

fn unique_temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("oxvba_{tag}_{}", std::process::id()))
}

fn vba_literal(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('"', "\"\"")
}

#[test]
fn mkdir_creates_a_real_directory() {
    let dir = unique_temp_dir("mkdir");
    let _ = std::fs::remove_dir_all(&dir);
    let source = format!("Sub Main()\n    MkDir \"{}\"\nEnd Sub\n", vba_literal(&dir));
    let result = run_clean(&source);
    let exists = dir.is_dir();
    let _ = std::fs::remove_dir_all(&dir); // teardown regardless of outcome
    result.expect("MkDir should succeed");
    assert!(exists, "MkDir did not create {}", dir.display());
}

#[test]
fn filecopy_and_filelen_round_trip() {
    // FileCopy duplicates a file; FileLen reports the copy's byte size; CurDir
    // returns a non-empty working directory. (Snapshot order: globals.)
    let dir = unique_temp_dir("fileops");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let src = dir.join("src.bin");
    let dst = dir.join("dst.bin");
    std::fs::write(&src, b"abcde").expect("seed src"); // 5 bytes
    let source = format!(
        "Public n As Long\nPublic cwd As String\n\
         Sub Main()\n    FileCopy \"{src}\", \"{dst}\"\n    n = FileLen(\"{dst}\")\n    cwd = CurDir()\nEnd Sub\n",
        src = vba_literal(&src),
        dst = vba_literal(&dst),
    );
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let snap = engine.execute_source_with_variant_snapshot_clean(&source);
    let copied = dst.is_file();
    let _ = std::fs::remove_dir_all(&dir);
    let snap = snap.unwrap_or_else(|d| panic!("{:?}: {}", d.phase(), d.message()));
    assert!(copied, "FileCopy did not create {}", dst.display());
    assert_eq!(snap[0].as_i32(), Some(5), "FileLen should be 5: {snap:?}");
    assert!(
        snap[1].as_bstr().is_some_and(|s| !s.as_str().is_empty()),
        "CurDir should be non-empty: {snap:?}"
    );
}

#[test]
fn rmdir_removes_an_empty_directory() {
    let dir = unique_temp_dir("rmdir");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir(&dir).expect("seed dir");
    let source = format!("Sub Main()\n    RmDir \"{}\"\nEnd Sub\n", vba_literal(&dir));
    let result = run_clean(&source);
    let still_exists = dir.exists();
    let _ = std::fs::remove_dir_all(&dir);
    result.expect("RmDir should succeed");
    assert!(!still_exists, "RmDir did not remove {}", dir.display());
}

fn clear_readonly(path: &std::path::Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        let mut perms = meta.permissions();
        // Windows-only test teardown: clearing the read-only bit so the temp file
        // can be deleted. The cross-platform caveat the lint warns about (Unix
        // mode bits) does not apply on this `#![cfg(target_os = "windows")]` lane.
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        let _ = std::fs::set_permissions(path, perms);
    }
}

#[test]
fn getattr_and_setattr_round_trip() {
    // GetAttr reports a directory's vbDirectory bit; SetAttr toggles vbReadOnly on
    // a file and GetAttr reflects it; SetAttr vbNormal clears it. (Snapshot order:
    // globals in declaration order.)
    let dir = unique_temp_dir("attrs");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let file = dir.join("a.txt");
    std::fs::write(&file, b"x").expect("seed file");
    let source = format!(
        "Public dirAttr As Long\nPublic roAttr As Long\nPublic clearedAttr As Long\n\
         Sub Main()\n\
         \u{20}   dirAttr = GetAttr(\"{dir}\")\n\
         \u{20}   SetAttr \"{file}\", vbReadOnly\n\
         \u{20}   roAttr = GetAttr(\"{file}\")\n\
         \u{20}   SetAttr \"{file}\", vbNormal\n\
         \u{20}   clearedAttr = GetAttr(\"{file}\")\n\
         End Sub\n",
        dir = vba_literal(&dir),
        file = vba_literal(&file),
    );
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let snap = engine.execute_source_with_variant_snapshot_clean(&source);
    clear_readonly(&file); // ensure teardown can delete it
    let _ = std::fs::remove_dir_all(&dir);
    let snap = snap.unwrap_or_else(|d| panic!("{:?}: {}", d.phase(), d.message()));
    assert_eq!(
        snap[0].as_i32().map(|v| v & 16),
        Some(16),
        "directory should carry vbDirectory: {snap:?}"
    );
    assert_eq!(
        snap[1].as_i32().map(|v| v & 1),
        Some(1),
        "file should be read-only after SetAttr vbReadOnly: {snap:?}"
    );
    assert_eq!(
        snap[2].as_i32().map(|v| v & 1),
        Some(0),
        "read-only should be cleared after SetAttr vbNormal: {snap:?}"
    );
}

#[test]
fn filedatetime_reads_a_files_modification_time() {
    // A just-written file's FileDateTime must sit within a minute of Now() — both
    // are built from the same UTC serial model, so this holds regardless of the
    // test machine's time zone, without asserting an absolute timestamp.
    let dir = unique_temp_dir("fdt");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let file = dir.join("recent.txt");
    std::fs::write(&file, b"x").expect("seed file");
    let source = format!(
        "Public gap As Double\n\
         Sub Main()\n    gap = Abs(Now() - FileDateTime(\"{file}\"))\nEnd Sub\n",
        file = vba_literal(&file),
    );
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let snap = engine.execute_source_with_variant_snapshot_clean(&source);
    let _ = std::fs::remove_dir_all(&dir);
    let snap = snap.unwrap_or_else(|d| panic!("{:?}: {}", d.phase(), d.message()));
    let gap_days = snap[0].as_f64().expect("gap is a Double");
    // One minute = 1/1440 of a day; a freshly written file is well within that.
    assert!(
        gap_days < 1.0 / 1440.0,
        "FileDateTime should be ~Now for a just-written file, gap was {gap_days} days"
    );
}

/// Seek/Loc are 1-based in VBA and the Seek STATEMENT is 1-based too. After
/// writing 3 bytes to a Binary file the next-write position `Seek(f)` is 4 and
/// the last-byte position `Loc(f)` is 3; `Seek #f, 2` then positions at the
/// 1-based 2nd byte, so `Get` reads it ('B'=66) and `Seek(f)` advances to 3.
/// (Live-Excel verified: bin3_seek=4 bin3_loc=3 readat2=66 ar_seek=3.)
#[test]
fn binary_seek_and_loc_are_one_based() {
    let dir = unique_temp_dir("seekbin");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let file = dir.join("b.bin");
    let source = format!(
        "Public sk As Long\nPublic lc As Long\nPublic lof3 As Long\n\
         Public skAfter As Long\nPublic readByte As Long\n\
         Sub Main()\n\
         \u{20}   Dim f As Integer, w As Byte, b As Byte\n\
         \u{20}   f = FreeFile\n\
         \u{20}   Open \"{file}\" For Binary As #f\n\
         \u{20}   w = 65: Put #f, , w\n\
         \u{20}   w = 66: Put #f, , w\n\
         \u{20}   w = 67: Put #f, , w\n\
         \u{20}   sk = Seek(f)\n\
         \u{20}   lc = Loc(f)\n\
         \u{20}   lof3 = LOF(f)\n\
         \u{20}   Seek #f, 2\n\
         \u{20}   Get #f, , b\n\
         \u{20}   readByte = b\n\
         \u{20}   skAfter = Seek(f)\n\
         \u{20}   Close #f\n\
         End Sub\n",
        file = vba_literal(&file),
    );
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let snap = engine.execute_source_with_variant_snapshot_clean(&source);
    let _ = std::fs::remove_dir_all(&dir);
    let snap = snap.unwrap_or_else(|d| panic!("{:?}: {}", d.phase(), d.message()));
    assert_eq!(
        snap[0].as_i32(),
        Some(4),
        "Seek after 3 bytes = 4: {snap:?}"
    );
    assert_eq!(snap[1].as_i32(), Some(3), "Loc after 3 bytes = 3: {snap:?}");
    assert_eq!(snap[2].as_i32(), Some(3), "LOF = 3: {snap:?}");
    assert_eq!(
        snap[3].as_i32(),
        Some(3),
        "Seek after reading byte 2 = 3: {snap:?}"
    );
    assert_eq!(
        snap[4].as_i32(),
        Some(66),
        "Get at 1-based byte 2 = 'B' (66): {snap:?}"
    );
}

/// A bare `Seek #f, pos` past EOF does NOT extend the file — only a subsequent
/// write grows it. `LOF` stays 1 and the on-disk file is still 1 byte, while the
/// reported `Seek(f)` is the requested 1-based position (10). (Live-Excel
/// verified: bin_seekpast_lof=1 bin_seekpast_seek=10 filelen=1.)
#[test]
fn seek_past_eof_does_not_extend_the_file() {
    let dir = unique_temp_dir("seekpast");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let file = dir.join("p.bin");
    let source = format!(
        "Public lofMid As Long\nPublic skMid As Long\n\
         Sub Main()\n\
         \u{20}   Dim f As Integer, w As Byte\n\
         \u{20}   f = FreeFile\n\
         \u{20}   Open \"{file}\" For Binary As #f\n\
         \u{20}   w = 65: Put #f, , w\n\
         \u{20}   Seek #f, 10\n\
         \u{20}   lofMid = LOF(f)\n\
         \u{20}   skMid = Seek(f)\n\
         \u{20}   Close #f\n\
         End Sub\n",
        file = vba_literal(&file),
    );
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let snap = engine.execute_source_with_variant_snapshot_clean(&source);
    let on_disk_len = std::fs::metadata(&file).map(|m| m.len()).ok();
    let _ = std::fs::remove_dir_all(&dir);
    let snap = snap.unwrap_or_else(|d| panic!("{:?}: {}", d.phase(), d.message()));
    assert_eq!(
        snap[0].as_i32(),
        Some(1),
        "LOF unchanged by bare seek: {snap:?}"
    );
    assert_eq!(
        snap[1].as_i32(),
        Some(10),
        "Seek reports the requested position: {snap:?}"
    );
    assert_eq!(
        on_disk_len,
        Some(1),
        "on-disk file must not be extended by a bare seek"
    );
}

/// In Random mode `Loc(f)` is the RECORD number of the last record (not a byte
/// offset) and `Seek(f)` is the next record. After writing two 4-byte records
/// Loc=2, Seek=3; after `Get #f, 1` Loc=1, Seek=2. (Live-Excel verified:
/// a2_seek=3 a2_loc=2 get1=100 g1_loc=1 g1_seek=2.)
#[test]
fn random_loc_and_seek_are_record_numbers() {
    let dir = unique_temp_dir("seekrnd");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let file = dir.join("r.dat");
    let source = format!(
        "Public skOpen As Long\nPublic lcOpen As Long\nPublic sk2 As Long\n\
         Public lc2 As Long\nPublic get1 As Long\nPublic lcGet As Long\nPublic skGet As Long\n\
         Sub Main()\n\
         \u{20}   Dim f As Integer, l As Long\n\
         \u{20}   f = FreeFile\n\
         \u{20}   Open \"{file}\" For Random As #f Len = 4\n\
         \u{20}   skOpen = Seek(f)\n\
         \u{20}   lcOpen = Loc(f)\n\
         \u{20}   l = 100: Put #f, , l\n\
         \u{20}   l = 200: Put #f, , l\n\
         \u{20}   sk2 = Seek(f)\n\
         \u{20}   lc2 = Loc(f)\n\
         \u{20}   l = 0: Get #f, 1, l\n\
         \u{20}   get1 = l\n\
         \u{20}   lcGet = Loc(f)\n\
         \u{20}   skGet = Seek(f)\n\
         \u{20}   Close #f\n\
         End Sub\n",
        file = vba_literal(&file),
    );
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let snap = engine.execute_source_with_variant_snapshot_clean(&source);
    let _ = std::fs::remove_dir_all(&dir);
    let snap = snap.unwrap_or_else(|d| panic!("{:?}: {}", d.phase(), d.message()));
    assert_eq!(
        snap[0].as_i32(),
        Some(1),
        "fresh Random Seek = record 1: {snap:?}"
    );
    assert_eq!(snap[1].as_i32(), Some(0), "fresh Random Loc = 0: {snap:?}");
    assert_eq!(
        snap[2].as_i32(),
        Some(3),
        "Seek after 2 records = 3: {snap:?}"
    );
    assert_eq!(
        snap[3].as_i32(),
        Some(2),
        "Loc after 2 records = record 2 (not byte 8): {snap:?}"
    );
    assert_eq!(snap[4].as_i32(), Some(100), "Get record 1 = 100: {snap:?}");
    assert_eq!(
        snap[5].as_i32(),
        Some(1),
        "Loc after Get record 1 = 1: {snap:?}"
    );
    assert_eq!(
        snap[6].as_i32(),
        Some(2),
        "Seek after Get record 1 = next record 2: {snap:?}"
    );
}

/// `Append` reports `Seek = 1` / `Loc = 0` on a fresh open even when the file is
/// non-empty, yet writes still land at EOF — the existing content is preserved
/// and the appended line follows it. (Live-Excel verified: app_seek=1 app_loc=0
/// with the appended data after the original.)
#[test]
fn append_reports_fresh_cursor_but_writes_at_eof() {
    let dir = unique_temp_dir("seekapp");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let file = dir.join("a.txt");
    let source = format!(
        "Public skFresh As Long\nPublic lcFresh As Long\n\
         Public firstLine As String\nPublic secondLine As String\n\
         Sub Main()\n\
         \u{20}   Dim f As Integer\n\
         \u{20}   f = FreeFile\n\
         \u{20}   Open \"{file}\" For Output As #f\n\
         \u{20}   Print #f, \"0123456789\"\n\
         \u{20}   Close #f\n\
         \u{20}   f = FreeFile\n\
         \u{20}   Open \"{file}\" For Append As #f\n\
         \u{20}   skFresh = Seek(f)\n\
         \u{20}   lcFresh = Loc(f)\n\
         \u{20}   Print #f, \"XY\"\n\
         \u{20}   Close #f\n\
         \u{20}   f = FreeFile\n\
         \u{20}   Open \"{file}\" For Input As #f\n\
         \u{20}   Line Input #f, firstLine\n\
         \u{20}   Line Input #f, secondLine\n\
         \u{20}   Close #f\n\
         End Sub\n",
        file = vba_literal(&file),
    );
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let snap = engine.execute_source_with_variant_snapshot_clean(&source);
    let _ = std::fs::remove_dir_all(&dir);
    let snap = snap.unwrap_or_else(|d| panic!("{:?}: {}", d.phase(), d.message()));
    assert_eq!(snap[0].as_i32(), Some(1), "fresh Append Seek = 1: {snap:?}");
    assert_eq!(snap[1].as_i32(), Some(0), "fresh Append Loc = 0: {snap:?}");
    assert_eq!(
        snap[2].as_bstr().map(|s| s.as_str().to_string()),
        Some("0123456789".to_string()),
        "Append must preserve the original first line: {snap:?}"
    );
    assert_eq!(
        snap[3].as_bstr().map(|s| s.as_str().to_string()),
        Some("XY".to_string()),
        "Append must land the new line at EOF: {snap:?}"
    );
}

#[test]
fn write_input_roundtrips_date_and_null_fields() {
    let dir = unique_temp_dir("writeinput_date_null");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let file = dir.join("records.txt");
    let source = format!(
        "Public gotDateText As String\nPublic gotNull As Boolean\n\
         Sub Main()\n\
         \u{20}   Dim f As Integer\n\
         \u{20}   Dim d As Date\n\
         \u{20}   Dim n As Variant\n\
         \u{20}   d = DateSerial(2020, 1, 15) + TimeSerial(13, 30, 5)\n\
         \u{20}   n = Null\n\
         \u{20}   f = FreeFile\n\
         \u{20}   Open \"{file}\" For Output As #f\n\
         \u{20}   Write #f, d, n\n\
         \u{20}   Close #f\n\
         \u{20}   d = 0\n\
         \u{20}   n = Empty\n\
         \u{20}   f = FreeFile\n\
         \u{20}   Open \"{file}\" For Input As #f\n\
         \u{20}   Input #f, d, n\n\
         \u{20}   Close #f\n\
         \u{20}   gotDateText = CStr(d)\n\
         \u{20}   gotNull = IsNull(n)\n\
         End Sub\n",
        file = vba_literal(&file),
    );
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let snap = engine.execute_source_with_variant_snapshot_clean(&source);
    let _ = std::fs::remove_dir_all(&dir);
    let snap = snap.unwrap_or_else(|d| panic!("{:?}: {}", d.phase(), d.message()));
    assert_eq!(
        snap[0].as_bstr().map(|s| s.as_str().to_string()),
        Some("1/15/2020 1:30:05 PM".to_string()),
        "Input # should parse Write # date literals: {snap:?}"
    );
    assert_eq!(
        snap[1].as_bool(),
        Some(true),
        "Input # should parse #NULL# as Null: {snap:?}"
    );
}

#[test]
fn print_hash_layout_residuals_match_vba_shape() {
    let dir = unique_temp_dir("printlayout");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let file = dir.join("layout.txt");
    let source = format!(
        "Sub Main()\n\
         \u{20}   Dim f As Integer\n\
         \u{20}   f = FreeFile\n\
         \u{20}   Open \"{file}\" For Output As #f\n\
         \u{20}   Print #f, \"abcd\";\n\
         \u{20}   Print #f, \"e\", \"z\"\n\
         \u{20}   Print #f, 1; -2; 0\n\
         \u{20}   Print #f, \"A\"; Spc(3); \"B\"; Tab(10); \"C\"; Tab; \"D\"\n\
         \u{20}   Close #f\n\
         End Sub\n",
        file = vba_literal(&file),
    );
    run_clean(&source).expect("Print # layout residuals should execute");
    let text = std::fs::read_to_string(&file).expect("read printed file");
    let _ = std::fs::remove_dir_all(&dir);
    let expected = format!(
        "abcde{}z\r\n 1 -2  0 \r\nA{}B{}C{}D\r\n",
        " ".repeat(9),
        " ".repeat(3),
        " ".repeat(4),
        " ".repeat(4)
    );
    assert_eq!(text, expected);
}

#[test]
fn width_hash_wraps_print_hash_like_vba() {
    let dir = unique_temp_dir("widthprint");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let file = dir.join("width.txt");
    let source = format!(
        "Sub Main()\n\
         \u{20}   Dim f As Integer\n\
         \u{20}   f = FreeFile\n\
         \u{20}   Open \"{file}\" For Output As #f\n\
         \u{20}   Width #f, 5\n\
         \u{20}   Print #f, \"ab\"; \"cd\"; \"ef\"\n\
         \u{20}   Print #f, 12; 34\n\
         \u{20}   Width #f, 10\n\
         \u{20}   Print #f, \"a\", \"b\"\n\
         \u{20}   Width #f, 5\n\
         \u{20}   Print #f, \"A\"; Spc(3); \"B\"; Tab(3); \"C\"; Tab; \"D\"\n\
         \u{20}   Print #f, Spc(6); \"A\"\n\
         \u{20}   Print #f, Tab(10); \"A\"\n\
         \u{20}   Width #f, 0\n\
         \u{20}   Print #f, \"abcdef\"\n\
         \u{20}   Write #f, \"abcdef\", 1\n\
         \u{20}   Close #f\n\
         End Sub\n",
        file = vba_literal(&file),
    );
    run_clean(&source).expect("Width # Print # wrapping should execute");
    let text = std::fs::read_to_string(&file).expect("read printed file");
    let _ = std::fs::remove_dir_all(&dir);
    let expected = concat!(
        "abcd\r\n",
        "ef\r\n",
        " 12 \r\n",
        " 34 \r\n",
        "a\r\n",
        "b\r\n",
        "A   B\r\n",
        "  C\r\n",
        "D\r\n",
        " A\r\n",
        "    A\r\n",
        "abcdef\r\n",
        "\"abcdef\",1\r\n"
    );
    assert_eq!(text, expected);
}

#[test]
fn width_hash_rejects_values_outside_vba_range() {
    let dir = unique_temp_dir("widthrange");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let file = dir.join("range.txt");
    let source = format!(
        "Public negNum As Long\nPublic negDesc As String\n\
         Public wideNum As Long\nPublic wideDesc As String\n\
         Sub Main()\n\
         \u{20}   Dim f As Integer\n\
         \u{20}   f = FreeFile\n\
         \u{20}   Open \"{file}\" For Output As #f\n\
         \u{20}   On Error Resume Next\n\
         \u{20}   Width #f, -1\n\
         \u{20}   negNum = Err.Number\n\
         \u{20}   negDesc = Err.Description\n\
         \u{20}   Err.Clear\n\
         \u{20}   Width #f, 256\n\
         \u{20}   wideNum = Err.Number\n\
         \u{20}   wideDesc = Err.Description\n\
         \u{20}   Close #f\n\
         End Sub\n",
        file = vba_literal(&file),
    );
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let snap = engine.execute_source_with_variant_snapshot_clean(&source);
    let _ = std::fs::remove_dir_all(&dir);
    let snap = snap.unwrap_or_else(|d| panic!("{:?}: {}", d.phase(), d.message()));
    assert_eq!(snap[0].as_i32(), Some(5), "negative width error: {snap:?}");
    assert_eq!(
        snap[1].as_bstr().map(|s| s.as_str().to_string()),
        Some("Invalid procedure call or argument".to_string()),
        "negative width description: {snap:?}"
    );
    assert_eq!(snap[2].as_i32(), Some(5), "width 256 error: {snap:?}");
    assert_eq!(
        snap[3].as_bstr().map(|s| s.as_str().to_string()),
        Some("Invalid procedure call or argument".to_string()),
        "width 256 description: {snap:?}"
    );
}

#[test]
fn width_hash_close_reopen_resets_width() {
    let dir = unique_temp_dir("widthreset");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let file = dir.join("reset.txt");
    let source = format!(
        "Sub Main()\n\
         \u{20}   Dim f As Integer\n\
         \u{20}   f = FreeFile\n\
         \u{20}   Open \"{file}\" For Output As #f\n\
         \u{20}   Width #f, 5\n\
         \u{20}   Close #f\n\
         \u{20}   f = FreeFile\n\
         \u{20}   Open \"{file}\" For Output As #f\n\
         \u{20}   Print #f, \"ab\"; \"cd\"; \"ef\"\n\
         \u{20}   Close #f\n\
         End Sub\n",
        file = vba_literal(&file),
    );
    run_clean(&source).expect("Width # should reset after close/reopen");
    let text = std::fs::read_to_string(&file).expect("read printed file");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(text, "abcdef\r\n");
}

/// For sequential output `Loc(f)` is `byte position \ 128`. Writing 200 bytes
/// gives Loc=1, 400 bytes gives Loc=3 and Seek=401. (Live-Excel verified:
/// w200_loc=1 w400_loc=3 w400_seek=401.)
#[test]
fn sequential_loc_is_byte_position_over_128() {
    let dir = unique_temp_dir("seekseq");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let file = dir.join("s.txt");
    let source = format!(
        "Public loc200 As Long\nPublic loc400 As Long\nPublic seek400 As Long\n\
         Sub Main()\n\
         \u{20}   Dim f As Integer, s As String\n\
         \u{20}   s = String(200, \"Z\")\n\
         \u{20}   f = FreeFile\n\
         \u{20}   Open \"{file}\" For Output As #f\n\
         \u{20}   Print #f, s;\n\
         \u{20}   loc200 = Loc(f)\n\
         \u{20}   Print #f, s;\n\
         \u{20}   loc400 = Loc(f)\n\
         \u{20}   seek400 = Seek(f)\n\
         \u{20}   Close #f\n\
         End Sub\n",
        file = vba_literal(&file),
    );
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let snap = engine.execute_source_with_variant_snapshot_clean(&source);
    let _ = std::fs::remove_dir_all(&dir);
    let snap = snap.unwrap_or_else(|d| panic!("{:?}: {}", d.phase(), d.message()));
    assert_eq!(
        snap[0].as_i32(),
        Some(1),
        "Loc after 200 bytes = 200\\128 = 1: {snap:?}"
    );
    assert_eq!(
        snap[1].as_i32(),
        Some(3),
        "Loc after 400 bytes = 400\\128 = 3: {snap:?}"
    );
    assert_eq!(
        snap[2].as_i32(),
        Some(401),
        "Seek after 400 bytes = 401: {snap:?}"
    );
}

#[test]
fn chdrive_runs_against_the_current_drive() {
    // ChDrive selects a drive's current directory. Drive a path on the current
    // drive and confirm it executes; save/restore the process CWD so sibling
    // tests (which all use absolute temp paths) are unaffected.
    let saved = std::env::current_dir().expect("cwd");
    let drive_letter = saved
        .to_string_lossy()
        .chars()
        .next()
        .expect("a drive letter")
        .to_string();
    let source = format!("Sub Main()\n    ChDrive \"{drive_letter}\"\nEnd Sub\n");
    let result = run_clean(&source);
    let _ = std::env::set_current_dir(&saved);
    result.expect("ChDrive on the current drive should succeed");
}
