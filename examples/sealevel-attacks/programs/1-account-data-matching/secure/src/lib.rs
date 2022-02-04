use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

declare_id!("Po1RaS8BEDbNcn5oXsFryAeQ6Wn8fvmE111DJaKCgPC");

#[program]
pub mod account_data_matching_recommended {
    use super::*;

    pub fn log_message(ctx: Context<LogMessage>) -> ProgramResult {
        msg!("Your acocunt balance is: {}", ctx.accounts.token.amount);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct LogMessage<'info> {
    // Instead, do this—Anchor checks that a token account contains valid data, 
    // and that its owner is the signer of transaction.
    #[account(constraint = authority.key == &token.owner)]
    token: Account<'info, TokenAccount>,
    authority: Signer<'info>,
}