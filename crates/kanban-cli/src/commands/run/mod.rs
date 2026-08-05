mod list;
mod log;
mod show;

pub(crate) use list::ListArgs;

use crate::{context::CliContext, error::CliFailure};

pub(crate) fn list(ctx: &CliContext, args: &ListArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    list::run(ctx, &client, args)
}
