use clap::{Args, CommandFactory};
use clap_complete::{Shell, generate};

use crate::Cli;

#[derive(Clone, Debug, Args)]
pub struct CompletionArgs {
    #[arg(help = "Shell to generate completions for")]
    shell: Shell,
}

pub fn handle(args: &CompletionArgs) -> anyhow::Result<()> {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();
    generate(args.shell, &mut cmd, &bin_name, &mut std::io::stdout());
    Ok(())
}
