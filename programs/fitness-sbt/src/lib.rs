#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;

declare_id!("8VX7TDKB4U1LR3RT8wYpF4kEUGuimgyJDnv6LwtFA1FB");

#[program]
pub mod fitness_sbt {
    use super::*;

    // ================================================================
    // Initialize Mint Config (rewards disabled by default)
    // ================================================================
    pub fn initialize_mint_config(ctx: Context<InitializeMintConfig>) -> Result<()> {
        let config = &mut ctx.accounts.mint_config;
        config.authority = ctx.accounts.authority.key();
        config.phase = 0;
        config.minted_phase1 = 0;
        config.minted_phase2 = 0;
        config.max_per_phase = 1000;
        config.current_image_uri = String::new();
        config.rewards_enabled = false;
        config.bump = ctx.bumps.mint_config;
        msg!("✅ Mint config initialized — rewards DISABLED by default");
        Ok(())
    }

    // ================================================================
    // Toggle rewards on/off (only us)
    // ================================================================
    pub fn toggle_rewards(ctx: Context<ManageMintConfig>, enable: bool) -> Result<()> {
        let config = &mut ctx.accounts.mint_config;
        require_keys_eq!(config.authority, ctx.accounts.authority.key(), ErrorCode::Unauthorized);
        config.rewards_enabled = enable;
        msg!("Rewards {}", if enable { "ENABLED" } else { "DISABLED" });
        Ok(())
    }

    // ================================================================
    // Claim Daily Reward (now with FIXED CPI seeds)
    // ================================================================
    pub fn claim_daily_reward(ctx: Context<ClaimDailyReward>, exercise_id: String) -> Result<()> {
        let config = &ctx.accounts.mint_config;
        require!(config.rewards_enabled, ErrorCode::RewardsNotEnabled);

        let transfer_amount: u64 = 10_000_000; // 0.01 SOL

        let cpi_accounts = anchor_lang::system_program::Transfer {
            from: ctx.accounts.reward_vault.to_account_info(),
            to: ctx.accounts.user.to_account_info(),
        };

        // ✅ FIXED: proper binding so seeds live long enough for CPI
        let seeds = [
            b"reward_vault".as_ref(),
            exercise_id.as_bytes(),
            &[ctx.bumps.reward_vault],
        ];
        let signer_seeds = [&seeds[..]];

        let cpi_ctx = CpiContext::new_with_signer(
            anchor_lang::system_program::ID,
            cpi_accounts,
            &signer_seeds,
        );

        anchor_lang::system_program::transfer(cpi_ctx, transfer_amount)?;

        msg!("✅ 0.01 SOL reward sent from vault for {}", exercise_id);
        Ok(())
    }

    // ================================================================
    // Log Workout (always works)
    // ================================================================
    pub fn log_workout(ctx: Context<LogWorkout>) -> Result<()> {
        let user_state = &mut ctx.accounts.user_state;
        user_state.workouts_logged = user_state.workouts_logged.saturating_add(1);
        msg!("✅ Workout logged for user {}", ctx.accounts.user.key());
        Ok(())
    }

    // ================================================================
    // DAILY NEWS (this is what fixes the revert on update/reset)
    // ================================================================
    pub fn update_daily_news(ctx: Context<UpdateDailyNews>, news_json: String) -> Result<()> {
        let daily_news = &mut ctx.accounts.daily_news;
        daily_news.posts_json = news_json;
        daily_news.bump = ctx.bumps.daily_news;
        msg!("✅ Daily news updated on-chain! JSON size: {} bytes", daily_news.posts_json.len());
        Ok(())
    }

    pub fn reset_daily_news(_ctx: Context<ResetDailyNews>) -> Result<()> {
        msg!("✅ Daily news PDA closed (ready for fresh init)");
        Ok(())
    }
}

// ================================================================
// Account structs
// ================================================================

#[account]
pub struct DailyNews {
    pub posts_json: String,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct UpdateDailyNews<'info> {
    #[account(
        init_if_needed,
        payer = authority,
        space = 8 + 4 + 2500 + 1,
        seeds = [b"daily-news-seeker-final"],
        bump
    )]
    pub daily_news: Account<'info, DailyNews>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResetDailyNews<'info> {
    #[account(
        mut,
        close = authority,
        seeds = [b"daily-news-seeker-final"],
        bump = daily_news.bump
    )]
    pub daily_news: Account<'info, DailyNews>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

// ================================================================
// Existing accounts (unchanged)
// ================================================================

#[derive(Accounts)]
pub struct InitializeMintConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 1 + 4 + 4 + 4 + 4 + 256 + 1 + 1,
        seeds = [b"mint-config"],
        bump
    )]
    pub mint_config: Account<'info, MintConfig>,
    #[account(mut)] pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ManageMintConfig<'info> {
    #[account(mut, seeds = [b"mint-config"], bump = mint_config.bump)]
    pub mint_config: Account<'info, MintConfig>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct LogWorkout<'info> {
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 32 + 32 + 8 + 4 + 1 + 1 + 1 + 32 + 8 + 1,
        seeds = [b"user-state", user.key().as_ref()],
        bump
    )]
    pub user_state: Account<'info, UserState>,
    #[account(mut)] pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct ClaimDailyReward<'info> {
    #[account(mut, seeds = [b"reward_vault", exercise_id.as_bytes()], bump)]
    pub reward_vault: SystemAccount<'info>,
    #[account(mut)] pub user: Signer<'info>,
    #[account(seeds = [b"mint-config"], bump = mint_config.bump)]
    pub mint_config: Account<'info, MintConfig>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct MintConfig {
    pub authority: Pubkey,
    pub phase: u8,
    pub minted_phase1: u32,
    pub minted_phase2: u32,
    pub max_per_phase: u32,
    pub current_image_uri: String,
    pub rewards_enabled: bool,
    pub bump: u8,
}

#[account]
pub struct UserState {
    pub owner: Pubkey,
    pub sbt_mint: Pubkey,
    pub total_calories: u64,
    pub workouts_logged: u32,
    pub minted_phase1: bool,
    pub minted_phase2: bool,
    pub bump: u8,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Unauthorized")] Unauthorized,
    #[msg("Rewards are not enabled yet")] RewardsNotEnabled,
}