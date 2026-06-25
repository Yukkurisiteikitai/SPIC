use ratatui::{backend::TestBackend, Terminal};
use slidecli::model::Presentation;
use slidecli::app::App;
use slidecli::ui::{draw, draw_exec_confirm, draw_present};

fn render(terminal: &mut Terminal<TestBackend>, app: &App, draw_fn: impl Fn(&mut ratatui::Frame<'_>, &App)) {
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

    println!("=== 通常エディタ（スライド3 execブロック選択）===");
    let mut t1 = Terminal::new(TestBackend::new(100, 30)).unwrap();
    render(&mut t1, &app, draw);

    println!("\n=== exec実行確認ダイアログ ===");
    let mut t2 = Terminal::new(TestBackend::new(100, 30)).unwrap();
    render(&mut t2, &app, draw_exec_confirm);

    println!("\n=== プレゼンモード ===");
    let mut t3 = Terminal::new(TestBackend::new(100, 30)).unwrap();
    render(&mut t3, &app, draw_present);
}
