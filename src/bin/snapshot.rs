use ratatui::{backend::TestBackend, Terminal};
use slidecli::app::{App, AppMode};
use slidecli::model::Presentation;
use slidecli::ui::{draw, draw_exec_confirm, draw_present, draw_present_exec_confirm};

fn render(
    terminal: &mut Terminal<TestBackend>,
    app: &App,
    draw_fn: impl Fn(&mut ratatui::Frame<'_>, &App),
) {
    terminal.draw(|f| draw_fn(f, app)).unwrap();
    let buf = terminal.backend().buffer().clone();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            print!("{}", buf.get(x, y).symbol);
        }
        println!();
    }
}

fn main() {
    let pres = Presentation::demo();
    let mut app = App::new(pres);
    app.go_to_slide(2);
    app.selected_block = Some(2); // execブロック
    app.current_slide_mut().blocks[3].content =
        "Compiling slidecli\nrunning 3 tests\ntest exec_flow ... ok\ntest present_flow ... ok\ntest output_render ... ok\nresult: ok"
            .to_string();

    println!("=== 通常エディタ（スライド3 execブロック選択）===");
    app.mode = AppMode::Normal;
    let mut t1 = Terminal::new(TestBackend::new(100, 30)).unwrap();
    render(&mut t1, &app, draw);

    println!("\n=== exec実行確認ダイアログ ===");
    app.mode = AppMode::ExecConfirm;
    let mut t2 = Terminal::new(TestBackend::new(100, 30)).unwrap();
    render(&mut t2, &app, draw_exec_confirm);

    println!("\n=== プレゼンモード ===");
    app.mode = AppMode::Present;
    let mut t3 = Terminal::new(TestBackend::new(100, 30)).unwrap();
    render(&mut t3, &app, draw_present);

    println!("\n=== プレゼン中exec実行確認ダイアログ ===");
    app.mode = AppMode::PresentExecConfirm;
    let mut t4 = Terminal::new(TestBackend::new(100, 30)).unwrap();
    render(&mut t4, &app, draw_present_exec_confirm);
}
