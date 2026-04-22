#[cfg(target_os = "windows")]
mod windows_file_io_host_backed_end_to_end {
    use oxvba_hal::model::HostPolicy;
    use oxvba_host::{Engine, HostConfig};
    use oxvba_runtime::{RuntimeValue, bstr::BStr};
    use std::fs;

    #[test]
    fn host_backed_file_print_line_input_roundtrip_returns_written_line() {
        let temp_dir = std::env::current_dir()
            .expect("cwd")
            .join("temp")
            .join("file-io-oracle-test");
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let temp_file = temp_dir.join("roundtrip.txt");
        let path_literal = temp_file.to_string_lossy().replace('\\', "\\\\");

        let mut engine = Engine::new(HostConfig::default());
        let mut policy = HostPolicy::interactive_dev();
        policy.allow_filesystem_mutation = true;
        engine.set_host_policy(policy);

        let source = format!(
            "Sub Main()\n\
             Dim a\n\
             Open \"{path_literal}\" For Output As #1\n\
             Print #1, \"world\"\n\
             Close #1\n\
             Open \"{path_literal}\" For Input As #2\n\
             Line Input #2, a\n\
             Close #2\n\
             End Sub"
        );
        let out = engine
            .execute_source_with_snapshot_phased(&source)
            .expect("host-backed file roundtrip should execute");
        println!(
            "ODG032-OBSERVED[CCT-033-LINE-001]={}",
            render_observed(&out[0])
        );
        assert_eq!(out[0], RuntimeValue::String(BStr::from("world")));
    }

    #[test]
    fn host_backed_file_eof_lof_seek_matches_excel_shape() {
        let temp_dir = std::env::current_dir()
            .expect("cwd")
            .join("temp")
            .join("file-io-oracle-test");
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let temp_file = temp_dir.join("filepos.txt");
        let path_literal = temp_file.to_string_lossy().replace('\\', "\\\\");

        let mut engine = Engine::new(HostConfig::default());
        let mut policy = HostPolicy::interactive_dev();
        policy.allow_filesystem_mutation = true;
        engine.set_host_policy(policy);

        let source = format!(
            "Sub Main()\n\
             Dim observed\n\
             Dim line\n\
             Open \"{path_literal}\" For Output As #1\n\
             Print #1, \"world\"\n\
             Close #1\n\
             Open \"{path_literal}\" For Input As #1\n\
             observed = CStr(EOF(1)) & \"|\" & CStr(LOF(1)) & \"|\" & CStr(Seek(1))\n\
             Line Input #1, line\n\
             observed = observed & \"|\" & line & \"|\" & CStr(EOF(1)) & \"|\" & CStr(Seek(1))\n\
             Close #1\n\
             End Sub"
        );
        let out = engine
            .execute_source_with_snapshot_phased(&source)
            .expect("host-backed file position/introspection case should execute");
        println!(
            "ODG032-OBSERVED[CCT-033-FILEPOS-001]={}",
            render_observed(&out[0])
        );
    }

    #[test]
    fn host_backed_file_write_input_preserves_embedded_comma_string() {
        let temp_dir = std::env::current_dir()
            .expect("cwd")
            .join("temp")
            .join("file-io-oracle-test");
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let temp_file = temp_dir.join("write_input.txt");
        let path_literal = temp_file.to_string_lossy().replace('\\', "\\\\");

        let mut engine = Engine::new(HostConfig::default());
        let mut policy = HostPolicy::interactive_dev();
        policy.allow_filesystem_mutation = true;
        engine.set_host_policy(policy);

        let source = format!(
            "Sub Main()\n\
             Dim a\n\
             Open \"{path_literal}\" For Output As #1\n\
             Write #1, \"hello,world\"\n\
             Close #1\n\
             Open \"{path_literal}\" For Input As #1\n\
             Input #1, a\n\
             Close #1\n\
             End Sub"
        );
        let out = engine
            .execute_source_with_snapshot_phased(&source)
            .expect("host-backed Write#/Input# case should execute");
        println!(
            "ODG032-OBSERVED[CCT-033-WRITE-001]={}",
            render_observed(&out[0])
        );
        assert_eq!(out[0], RuntimeValue::String(BStr::from("hello,world")));
    }

    #[test]
    fn host_backed_file_write_input_multi_field_typed_roundtrip_matches_excel_shape() {
        let temp_dir = std::env::current_dir()
            .expect("cwd")
            .join("temp")
            .join("file-io-oracle-test");
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let temp_file = temp_dir.join("write_input_multi.txt");
        let path_literal = temp_file.to_string_lossy().replace('\\', "\\\\");

        let mut engine = Engine::new(HostConfig::default());
        let mut policy = HostPolicy::interactive_dev();
        policy.allow_filesystem_mutation = true;
        engine.set_host_policy(policy);

        let source = format!(
            "Sub Main()\n\
             Dim a\n\
             Dim b\n\
             Dim c\n\
             Dim observed\n\
             Open \"{path_literal}\" For Output As #1\n\
             Write #1, 42, True, \"hello,world\"\n\
             Close #1\n\
             Open \"{path_literal}\" For Input As #1\n\
             Input #1, a, b, c\n\
             Close #1\n\
             observed = CStr(a) & \"|\" & CStr(b) & \"|\" & CStr(c)\n\
             End Sub"
        );
        let out = engine
            .execute_source_with_snapshot_phased(&source)
            .expect("host-backed multi-field Write#/Input# case should execute");
        let observed = out.last().expect("expected observed slot");
        println!(
            "ODG032-OBSERVED[CCT-033-WRITE-002]={}",
            render_observed(observed)
        );
        assert_eq!(
            observed,
            &RuntimeValue::String(BStr::from("42|True|hello,world"))
        );
    }

    #[test]
    fn host_backed_kill_deletes_closed_file() {
        let temp_dir = std::env::current_dir()
            .expect("cwd")
            .join("temp")
            .join("file-io-oracle-test");
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let temp_file = temp_dir.join("kill_me.txt");
        let path_literal = temp_file.to_string_lossy().replace('\\', "\\\\");

        let mut engine = Engine::new(HostConfig::default());
        let mut policy = HostPolicy::interactive_dev();
        policy.allow_filesystem_mutation = true;
        engine.set_host_policy(policy);

        let source = format!(
            "Sub Main()\n\
             Open \"{path_literal}\" For Output As #1\n\
             Print #1, \"world\"\n\
             Close #1\n\
             Kill \"{path_literal}\"\n\
             End Sub"
        );
        engine
            .execute_source_with_snapshot_phased(&source)
            .expect("host-backed Kill should execute");
        assert!(
            !temp_file.exists(),
            "Kill should remove the host-backed file at {}",
            temp_file.display()
        );
    }

    #[test]
    fn host_backed_kill_deletes_wildcard_matches_in_single_directory() {
        let temp_dir = std::env::current_dir()
            .expect("cwd")
            .join("temp")
            .join("file-io-oracle-test")
            .join("wildcard-kill");
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let txt_a = temp_dir.join("kill_a.txt");
        let txt_b = temp_dir.join("kill_b.txt");
        let keep_log = temp_dir.join("keep.log");
        fs::write(&txt_a, "a").expect("seed txt_a");
        fs::write(&txt_b, "b").expect("seed txt_b");
        fs::write(&keep_log, "c").expect("seed keep_log");
        let wildcard_literal = temp_dir
            .join("kill_?.txt")
            .to_string_lossy()
            .replace('\\', "\\\\");

        let mut engine = Engine::new(HostConfig::default());
        let mut policy = HostPolicy::interactive_dev();
        policy.allow_filesystem_mutation = true;
        engine.set_host_policy(policy);

        let source = format!("Sub Main()\nKill \"{wildcard_literal}\"\nEnd Sub");
        engine
            .execute_source_with_snapshot_phased(&source)
            .expect("host-backed wildcard Kill should execute");
        assert!(
            !txt_a.exists(),
            "wildcard Kill should remove {}",
            txt_a.display()
        );
        assert!(
            !txt_b.exists(),
            "wildcard Kill should remove {}",
            txt_b.display()
        );
        assert!(
            keep_log.exists(),
            "wildcard Kill should leave non-matching files alone at {}",
            keep_log.display()
        );
    }

    #[test]
    fn host_backed_dir_wildcard_enumerates_matches_and_exhausts() {
        let temp_dir = std::env::current_dir()
            .expect("cwd")
            .join("temp")
            .join("file-io-oracle-test")
            .join("dir-wildcard");
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        fs::write(temp_dir.join("alpha.txt"), "a").expect("seed alpha");
        fs::write(temp_dir.join("apple.txt"), "b").expect("seed apple");
        fs::write(temp_dir.join("beta.log"), "c").expect("seed beta");
        let wildcard_literal = temp_dir
            .join("a*.txt")
            .to_string_lossy()
            .replace('\\', "\\\\");

        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = format!(
            "Sub Main()\n\
             Dim a\n\
             Dim b\n\
             Dim c\n\
             a = Dir(\"{wildcard_literal}\")\n\
             b = Dir()\n\
             c = Dir()\n\
             End Sub"
        );
        let out = engine
            .execute_source_with_snapshot_phased(&source)
            .expect("host-backed Dir wildcard enumeration should execute");
        assert_eq!(
            out,
            vec![
                RuntimeValue::String(BStr::from("alpha.txt")),
                RuntimeValue::String(BStr::from("apple.txt")),
                RuntimeValue::String(BStr::empty())
            ]
        );
    }

    #[test]
    fn host_backed_dir_wildcard_expands_parent_segments() {
        let temp_dir = std::env::current_dir()
            .expect("cwd")
            .join("temp")
            .join("file-io-oracle-test")
            .join("dir-parent-wildcard");
        let branch_a = temp_dir.join("branch-one");
        let branch_b = temp_dir.join("branch-two");
        fs::create_dir_all(&branch_a).expect("create branch a");
        fs::create_dir_all(&branch_b).expect("create branch b");
        fs::write(branch_a.join("alpha.txt"), "a").expect("seed branch a");
        fs::write(branch_b.join("apple.txt"), "b").expect("seed branch b");
        let wildcard_literal = temp_dir
            .join("branch-*")
            .join("a*.txt")
            .to_string_lossy()
            .replace('\\', "\\\\");

        let mut engine = Engine::new(HostConfig::default());
        engine.set_host_policy(HostPolicy::interactive_dev());

        let source = format!(
            "Sub Main()\n\
             Dim a\n\
             Dim b\n\
             Dim c\n\
             a = Dir(\"{wildcard_literal}\")\n\
             b = Dir()\n\
             c = Dir()\n\
             End Sub"
        );
        let out = engine
            .execute_source_with_snapshot_phased(&source)
            .expect("host-backed Dir parent wildcard enumeration should execute");
        assert_eq!(
            out,
            vec![
                RuntimeValue::String(BStr::from("alpha.txt")),
                RuntimeValue::String(BStr::from("apple.txt")),
                RuntimeValue::String(BStr::empty())
            ]
        );
    }

    #[test]
    fn host_backed_kill_deletes_wildcard_matches_across_parent_segments() {
        let temp_dir = std::env::current_dir()
            .expect("cwd")
            .join("temp")
            .join("file-io-oracle-test")
            .join("kill-parent-wildcard");
        let branch_a = temp_dir.join("branch-one");
        let branch_b = temp_dir.join("branch-two");
        fs::create_dir_all(&branch_a).expect("create branch a");
        fs::create_dir_all(&branch_b).expect("create branch b");
        let kill_a = branch_a.join("kill_a.txt");
        let kill_b = branch_b.join("kill_b.txt");
        let keep_a = branch_a.join("keep.log");
        fs::write(&kill_a, "a").expect("seed kill_a");
        fs::write(&kill_b, "b").expect("seed kill_b");
        fs::write(&keep_a, "c").expect("seed keep_a");
        let wildcard_literal = temp_dir
            .join("branch-*")
            .join("kill_?.txt")
            .to_string_lossy()
            .replace('\\', "\\\\");

        let mut engine = Engine::new(HostConfig::default());
        let mut policy = HostPolicy::interactive_dev();
        policy.allow_filesystem_mutation = true;
        engine.set_host_policy(policy);

        let source = format!("Sub Main()\nKill \"{wildcard_literal}\"\nEnd Sub");
        engine
            .execute_source_with_snapshot_phased(&source)
            .expect("host-backed parent wildcard Kill should execute");
        assert!(
            !kill_a.exists(),
            "wildcard Kill should remove {}",
            kill_a.display()
        );
        assert!(
            !kill_b.exists(),
            "wildcard Kill should remove {}",
            kill_b.display()
        );
        assert!(
            keep_a.exists(),
            "wildcard Kill should leave non-matching files alone at {}",
            keep_a.display()
        );
    }

    fn render_observed(value: &RuntimeValue) -> String {
        match value {
            RuntimeValue::String(text) => text.as_str().to_string(),
            RuntimeValue::Bool(value) => value.to_string(),
            other => format!("{other:?}"),
        }
    }
}
