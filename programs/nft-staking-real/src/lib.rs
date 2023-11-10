use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_spl::metadata::mpl_token_metadata::instructions::FreezeDelegatedAccount;
use anchor_spl::metadata::mpl_token_metadata::instructions::ThawDelegatedAccount;
use anchor_spl::metadata::mpl_token_metadata::ID as MetadataTokenId;
use anchor_spl::token;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Approve, Mint, MintTo, Revoke, Token, TokenAccount},
};

declare_id!("Astt3W2ShtbDKiKK3LJqcVgfuMKDwmho62JpPNasW9r2");

#[program]
pub mod nft_staking_real {
    use super::*;

    pub fn stake(ctx: Context<Stake>) -> Result<()> {
        require!(
            ctx.accounts.stake_state.stake_state == StakeState::Unstaked,
            StakeError::AlreadyStaked
        );

        msg!("Stake called!");
        let clock = Clock::get().unwrap();

        msg!("Approving delegate");
        let cpi_approve_program = ctx.accounts.token_program.to_account_info();
        let cpi_approve_accounts: Approve<'_> = Approve {
            to: ctx.accounts.nft_token_account.to_account_info(),
            delegate: ctx.accounts.program_authority.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_approve_ctx = CpiContext::new(cpi_approve_program, cpi_approve_accounts);
        token::approve(cpi_approve_ctx, 1)?;

        msg!("Freezing token account");
        let authority_bump = ctx.bumps.program_authority;

        let cpi_accounts = FreezeDelegatedAccount {
            delegate: ctx.accounts.program_authority.key(),
            token_account: ctx.accounts.nft_token_account.key(),
            edition: ctx.accounts.nft_edition.key(),
            mint: ctx.accounts.nft_mint.key(),
            token_program: ctx.accounts.token_program.key(),
        };

        let instruction = cpi_accounts.instruction();
        let seeds = &["authority".as_bytes(), &[authority_bump]];
        let signer = &[&seeds[..]];
        let account_infos = vec![
            ctx.accounts.program_authority.to_account_info(),
            ctx.accounts.nft_token_account.to_account_info(),
            ctx.accounts.nft_edition.to_account_info(),
            ctx.accounts.nft_mint.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ];
        invoke_signed(&instruction, &account_infos, signer)?;

        msg!("Saving state");
        ctx.accounts.stake_state.token_account = ctx.accounts.nft_token_account.key();
        ctx.accounts.stake_state.user_pubkey = ctx.accounts.user.key();
        ctx.accounts.stake_state.stake_state = StakeState::Staked;
        ctx.accounts.stake_state.stake_start_time = clock.unix_timestamp;
        ctx.accounts.stake_state.last_stake_redeem = clock.unix_timestamp;
        ctx.accounts.stake_state.is_initialized = true;

        msg!("Stake state: {:?}", ctx.accounts.stake_state.stake_state);

        Ok(())
    }

    pub fn redeem(ctx: Context<Redeem>) -> Result<()> {
        helpers::process_redeem(ctx.accounts, ctx.bumps.stake_authority)?;
        Ok(())
    }

    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        msg!("Unstake called!");
        let mut redeem_account = Redeem {
            user: ctx.accounts.user.clone(),
            nft_token_account: ctx.accounts.nft_token_account.clone(),
            stake_state: ctx.accounts.stake_state.clone(),
            stake_mint: ctx.accounts.stake_mint.clone(),
            stake_authority: ctx.accounts.stake_authority.clone(),
            user_stake_ata: ctx.accounts.user_stake_ata.clone(),
            token_program: ctx.accounts.token_program.clone(),
            system_program: ctx.accounts.system_program.clone(),
            rent: ctx.accounts.rent.clone(),
            associated_token_program: ctx.accounts.associated_token_program.clone(),
        };
        helpers::process_redeem(&mut redeem_account, ctx.bumps.stake_authority)?;

        // Update state
        ctx.accounts.stake_state.stake_state = StakeState::Unstaked;

        msg!("Thawing token");
        let cpi_accounts = ThawDelegatedAccount {
            delegate: ctx.accounts.program_authority.key(),
            token_account: ctx.accounts.nft_token_account.key(),
            edition: ctx.accounts.nft_edition.key(),
            mint: ctx.accounts.nft_mint.key(),
            token_program: ctx.accounts.token_program.key(),
        };
        let instruction = cpi_accounts.instruction();
        let seeds = &["authority".as_bytes(), &[ctx.bumps.program_authority]];
        let signer = &[&seeds[..]];
        let account_infos = vec![
            ctx.accounts.program_authority.to_account_info(),
            ctx.accounts.nft_token_account.to_account_info(),
            ctx.accounts.nft_edition.to_account_info(),
            ctx.accounts.nft_mint.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ];
        invoke_signed(&instruction, &account_infos, signer)?;

        // Revoking delegation
        msg!("Revoking delegate");

        let cpi_revoke_program = ctx.accounts.token_program.to_account_info();
        let cpi_revoke_accounts = Revoke {
            source: ctx.accounts.nft_token_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };

        let cpi_revoke_ctx = CpiContext::new(cpi_revoke_program, cpi_revoke_accounts);
        token::revoke(cpi_revoke_ctx)?;

        Ok(())
    }
}

mod helpers {
    use super::*;

    // Internal helper function
    pub(crate) fn process_redeem(
        redeem_account: &mut Redeem,
        stake_authority_bump: u8,
    ) -> Result<()> {
        require!(
            redeem_account.stake_state.is_initialized,
            StakeError::UninitializedAccount
        );

        require!(
            redeem_account.stake_state.stake_state == StakeState::Staked,
            StakeError::InvalidStakeState
        );

        let clock = Clock::get()?;
        msg!(
            "Stake last redeem: {:?}",
            redeem_account.stake_state.last_stake_redeem
        );

        msg!("Current time: {:?}", clock.unix_timestamp);
        let unix_time = clock.unix_timestamp - redeem_account.stake_state.last_stake_redeem;
        msg!("Seconds since last redeem: {}", unix_time);
        let redeem_amount = (10 * i64::pow(10, 2) * unix_time) / (24 * 60 * 60);
        msg!("Eligible redeem amount: {}", redeem_amount);

        msg!("Minting reward tokens");
        let cpi_mint_program = redeem_account.token_program.to_account_info();
        let cpi_mint_accounts: MintTo<'_> = MintTo {
            mint: redeem_account.stake_mint.to_account_info(),
            to: redeem_account.user_stake_ata.to_account_info(),
            authority: redeem_account.stake_authority.to_account_info(),
        };
        let seeds = &["mint".as_bytes(), &[stake_authority_bump]];
        let signer = &[&seeds[..]];
        let cpi_mint_ctx = CpiContext::new_with_signer(cpi_mint_program, cpi_mint_accounts, signer);
        token::mint_to(cpi_mint_ctx, redeem_amount as u64)?;

        redeem_account.stake_state.last_stake_redeem = clock.unix_timestamp;
        msg!(
            "Updated last stake redeem time: {:?}",
            redeem_account.stake_state.last_stake_redeem
        );

        Ok(())
    }
}

// Accounts
#[account]
pub struct UserStakeInfo {
    pub token_account: Pubkey,
    pub stake_start_time: i64,
    pub last_stake_redeem: i64,
    pub user_pubkey: Pubkey,
    pub stake_state: StakeState,
    pub is_initialized: bool,
}

// Instructions
#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        associated_token::mint=nft_mint,
        associated_token::authority=user
    )]
    pub nft_token_account: Account<'info, TokenAccount>,
    pub nft_mint: Account<'info, Mint>,
    /// CHECK: Manual validation
    #[account(owner=MetadataTokenId)]
    pub nft_edition: UncheckedAccount<'info>,
    #[account(
        init_if_needed,
        payer=user,
        space = std::mem::size_of::<UserStakeInfo>() + 8,
        seeds = [user.key().as_ref(), nft_token_account.key().as_ref()],
        bump
    )]
    pub stake_state: Account<'info, UserStakeInfo>,
    /// CHECK: Manual validation
    #[account(mut, seeds=["authority".as_bytes().as_ref()], bump)]
    pub program_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub metadata_program: Program<'info, Metadata>,
}

#[derive(Accounts)]
pub struct Redeem<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, token::authority=user)]
    pub nft_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [user.key().as_ref(), nft_token_account.key().as_ref()],
        bump,
        constraint = *user.key == stake_state.user_pubkey,
        constraint = nft_token_account.key() == stake_state.token_account
    )]
    pub stake_state: Account<'info, UserStakeInfo>,
    #[account(mut)]
    pub stake_mint: Account<'info, Mint>, // reward spl token
    /// CHECK: manual check
    #[account(seeds = ["mint".as_bytes().as_ref()], bump)]
    pub stake_authority: UncheckedAccount<'info>,
    #[account(
        init_if_needed,
        payer=user,
        associated_token::mint=stake_mint,
        associated_token::authority=user
    )]
    pub user_stake_ata: Account<'info, TokenAccount>, // reward spl token ata
    // Non-optional
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        token::authority=user
    )]
    pub nft_token_account: Account<'info, TokenAccount>,
    pub nft_mint: Account<'info, Mint>,
    /// CHECK: Manual validation
    #[account(owner=MetadataTokenId)]
    pub nft_edition: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [user.key().as_ref(), nft_token_account.key().as_ref()],
        bump,
        constraint = *user.key == stake_state.user_pubkey,
        constraint = nft_token_account.key() == stake_state.token_account
    )]
    pub stake_state: Account<'info, UserStakeInfo>,
    /// CHECK: manual check
    #[account(mut, seeds=["authority".as_bytes().as_ref()], bump)]
    pub program_authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub stake_mint: Account<'info, Mint>,
    /// CHECK: manual check
    #[account(seeds = ["mint".as_bytes().as_ref()], bump)]
    pub stake_authority: UncheckedAccount<'info>,
    #[account(
        init_if_needed,
        payer=user,
        associated_token::mint=stake_mint,
        associated_token::authority=user
    )]
    pub user_stake_ata: Account<'info, TokenAccount>,
    // Default
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
    pub metadata_program: Program<'info, Metadata>,
}

// Enums
#[derive(Debug, PartialEq, AnchorDeserialize, AnchorSerialize, Clone)]
pub enum StakeState {
    Unstaked,
    Staked,
}

impl Default for StakeState {
    fn default() -> Self {
        StakeState::Unstaked
    }
}

#[derive(Clone)]
pub struct Metadata;

impl anchor_lang::Id for Metadata {
    fn id() -> Pubkey {
        MetadataTokenId
    }
}

// Errors
#[error_code]
pub enum StakeError {
    #[msg("NFT already staked")]
    AlreadyStaked,
    #[msg("State account is uninitialized")]
    UninitializedAccount,
    #[msg("Stake state is invalid")]
    InvalidStakeState,
}
