use std::thread;
use std::time::{Duration, Instant};

use slidecli::app::{App, ExecStatus};
use slidecli::model::Presentation;

fn drive(app: &mut App, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        app.poll_exec_events();
        if let Some(r) = &app.running_exec {
            if !matches!(r.status, ExecStatus::Running) && r.notified {
                app.poll_exec_events();
                break;
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn main() {
    println!("--- smoke_exec ---");

    let pres = Presentation::demo();
    let mut app = App::new(pres);
    app.go_to_slide(2);
    app.selected_block = Some(2);

    {
        let slide = app.current_slide_mut();
        slide.blocks[2].content = "for i in 1 2 3; do echo line$i; sleep 0.2; done".to_string();
    }

    app.run_exec_selected();
    assert!(app.running_exec.is_some(), "spawn failed");
    println!("[1] streaming run_exec_selected: 起動 OK");

    drive(&mut app, Duration::from_secs(3));
    let r = app.running_exec.as_ref().unwrap();
    println!("  buffer:\n{}", indent(&r.buffer));
    assert!(r.buffer.contains("line1") && r.buffer.contains("line2") && r.buffer.contains("line3"),
            "missing output lines");
    assert!(matches!(r.status, ExecStatus::Completed(0)), "expected exit 0, got {:?}", r.status);

    let ph = &app.current_slide().blocks[3];
    assert!(ph.content.contains("line3"), "placeholder not updated");
    println!("[1] OutputPlaceholder 更新 OK");

    {
        let slide = app.current_slide_mut();
        slide.blocks[2].content = "sleep 5".to_string();
    }
    app.run_exec_selected();
    assert!(app.running_exec.is_some());
    thread::sleep(Duration::from_millis(200));
    app.cancel_running_exec();
    let r = app.running_exec.as_ref().unwrap();
    assert!(matches!(r.status, ExecStatus::Cancelled), "expected Cancelled, got {:?}", r.status);
    assert!(r.buffer.contains("キャンセル"));
    println!("[2] cancel_running_exec: Cancelled OK");

    {
        let slide = app.current_slide_mut();
        slide.blocks[2].content = "false".to_string();
    }
    app.run_exec_selected();
    drive(&mut app, Duration::from_secs(3));
    let r = app.running_exec.as_ref().unwrap();
    assert!(matches!(r.status, ExecStatus::Failed(1)), "expected Failed(1), got {:?}", r.status);
    println!("[3] 失敗 exit 1 検出 OK");

    {
        let slide = app.current_slide_mut();
        slide.blocks[2].content = "echo hello".to_string();
    }
    app.run_exec_selected();
    drive(&mut app, Duration::from_secs(3));
    let r = app.running_exec.as_ref().unwrap();
    assert!(matches!(r.status, ExecStatus::Completed(0)));
    assert!(r.buffer.contains("hello"));
    println!("[4] 終了済みステートからの再実行 OK");

    println!("\n全テスト通過");
}

fn indent(s: &str) -> String {
    s.lines().map(|l| format!("    {}", l)).collect::<Vec<_>>().join("\n")
}
