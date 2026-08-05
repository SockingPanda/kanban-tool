mod list;

pub(crate) use list::ListArgs;

use crate::{context::CliContext, error::CliFailure};

pub(crate) fn run(ctx: &CliContext, args: &ListArgs) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    list::run(ctx, &client, args)
}
