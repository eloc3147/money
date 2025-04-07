use money::importer::app::ImporterApp;

use color_eyre::Result;

fn main() -> Result<()> {
    color_eyre::install()?;

    let mut terminal = ratatui::init();
    let result = ImporterApp::new().run(&mut terminal);
    ratatui::restore();

    result
}
