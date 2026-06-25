use ratatui::{backend::TestBackend, Terminal};
use slidecli::app::{App, AppMode};
use slidecli::model::Presentation;
use slidecli::ui::{
    draw, draw_command_input, draw_exec_confirm, draw_present, draw_present_exec_confirm,
    draw_settings,
};

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

    println!("\n=== 通常エディタ（font-size 50）===");
    app.presentation.font_size = 50;
    let mut t1_zoom = Terminal::new(TestBackend::new(100, 30)).unwrap();
    render(&mut t1_zoom, &app, draw);
    app.presentation.font_size = 14;

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

    println!("\n=== プレゼンモード（長文自動フィット）===");
    app.go_to_slide(1);
    app.current_slide_mut().blocks[1].content =
        "• Markdown互換の内部ブロックモデルを保ちながら、編集時はブロック単位で扱う\n• execブロックは署名済みでも必ず実行確認を通し、発表中も同じ安全規則を維持する\n• 出力プレースホルダは長い実行結果を必要な分だけ表示し、スライド全体の比率を崩さない\n• 画面サイズと本文量に応じて余白・幅・行間・出力行数を自動調節する"
            .to_string();
    app.mode = AppMode::Present;
    let mut t5 = Terminal::new(TestBackend::new(100, 30)).unwrap();
    render(&mut t5, &app, draw_present);

    println!("\n=== 設定画面 ===");
    app.mode = AppMode::Settings;
    let mut t6 = Terminal::new(TestBackend::new(100, 30)).unwrap();
    render(&mut t6, &app, draw_settings);

    println!("\n=== 設定コマンド入力 ===");
    app.mode = AppMode::CommandInput;
    app.command_return_mode = AppMode::Settings;
    app.command_buffer = "font-size 20".to_string();
    app.command_cursor = app.command_buffer.len();
    let mut t7 = Terminal::new(TestBackend::new(100, 30)).unwrap();
    render(&mut t7, &app, draw_command_input);
}
