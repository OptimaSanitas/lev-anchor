#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_spl::token::{self, MintTo};

declare_id!("5PwEGd5ndktrTHTTMpbbvmaqP2piChGgTgdyWP9Us9y6");

#[program]
pub mod fitness_sbt {
    use super::*;

    // ====================== ADMIN FUNCTIONS ======================
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

        msg!("✅ Mint config initialized successfully");
        Ok(())
    }

    pub fn toggle_rewards(ctx: Context<ManageMintConfig>, enable: bool) -> Result<()> {
        let config = &mut ctx.accounts.mint_config;
        require_keys_eq!(config.authority, ctx.accounts.authority.key(), ErrorCode::Unauthorized);
        config.rewards_enabled = enable;
        msg!("Rewards {}", if enable { "ENABLED" } else { "DISABLED" });
        Ok(())
    }

    // ====================== DAILY NEWS ======================
    pub fn update_daily_news(ctx: Context<UpdateDailyNews>, news_json: String) -> Result<()> {
        require!(news_json.len() <= 34000, ErrorCode::NewsTooLarge);

        let _: Vec<NewsThread> = serde_json::from_str(&news_json)
            .map_err(|_| error!(ErrorCode::InvalidJson))?;

        let daily_news = &mut ctx.accounts.daily_news;
        daily_news.posts_json = news_json;
        daily_news.bump = ctx.bumps.daily_news;

        msg!("✅ Daily news updated — ring buffer up to 13 threads");
        Ok(())
    }

    pub fn reset_daily_news(_ctx: Context<ResetDailyNews>) -> Result<()> {
        msg!("✅ Daily news PDA closed");
        Ok(())
    }

    // ====================== WORKOUT & REWARDS ======================
    pub fn log_workout(ctx: Context<LogWorkout>) -> Result<()> {
        let user_state = &mut ctx.accounts.user_state;
        user_state.workouts_logged = user_state.workouts_logged.saturating_add(1);
        msg!("✅ Workout logged");
        Ok(())
    }

 // === CLAIM DAILY SKR (now transfers from reward_vault ATA — matches claim_daily_reward pattern) ===
pub fn claim_daily_skr(ctx: Context<ClaimDailySKR>) -> Result<()> {
    let config = &ctx.accounts.mint_config;
    require!(config.rewards_enabled, ErrorCode::RewardsNotEnabled);

    let transfer_amount: u64 = 50_000_000; // 0.05 SKR (9 decimals)

    let cpi_accounts = anchor_spl::token::Transfer {
        from: ctx.accounts.reward_vault_ata.to_account_info(),
        to: ctx.accounts.user_token_account.to_account_info(),
        authority: ctx.accounts.reward_vault.to_account_info(),
    };

    let seeds = [b"reward_vault".as_ref(), b"skr".as_ref(), &[ctx.bumps.reward_vault]];
    let signer_seeds = [&seeds[..]];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        cpi_accounts,
        &signer_seeds,
    );

    anchor_spl::token::transfer(cpi_ctx, transfer_amount)?;

    msg!("✅ 0.05 SKR transferred from reward vault");

    // Legend SBT logic (unchanged)
    if !ctx.accounts.user_state.minted_phase1 && config.minted_phase1 < 1800 {
        let config_mut = &mut ctx.accounts.mint_config;
        config_mut.minted_phase1 += 1;
        ctx.accounts.user_state.minted_phase1 = true;
        msg!("🏆 Legend SBT #{} minted!", config_mut.minted_phase1);
    }

    Ok(())
}


// === ADD INSIDE pub mod fitness_sbt { ... } (right after claim_daily_skr for example) ===
    pub fn admin_mint_skr(ctx: Context<AdminMintSKR>, amount: u64) -> Result<()> {
        let config = &ctx.accounts.mint_config;
        require_keys_eq!(config.authority, ctx.accounts.authority.key(), ErrorCode::Unauthorized);

        let cpi_accounts = MintTo {
            mint: ctx.accounts.skr_mint.to_account_info(),
            to: ctx.accounts.destination.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.key(), cpi_accounts);
        token::mint_to(cpi_ctx, amount)?;

        msg!("✅ ADMIN: Minted {} SKR to test wallet", amount / 1_000_000_000);
        Ok(())
    }
    


    pub fn claim_daily_reward(ctx: Context<ClaimDailyReward>, exercise_id: String) -> Result<()> {
        let config = &ctx.accounts.mint_config;
        require!(config.rewards_enabled, ErrorCode::RewardsNotEnabled);

        let transfer_amount: u64 = 10_000_000; // 0.01 SOL

        let cpi_accounts = anchor_lang::system_program::Transfer {
            from: ctx.accounts.reward_vault.to_account_info(),
            to: ctx.accounts.user.to_account_info(),
        };

        let seeds = [b"reward_vault".as_ref(), exercise_id.as_bytes(), &[ctx.bumps.reward_vault]];
        let signer_seeds = [&seeds[..]];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.system_program.key(),   // ← Fixed: use .key() (Pubkey)
            cpi_accounts,
            &signer_seeds,
        );

        anchor_lang::system_program::transfer(cpi_ctx, transfer_amount)?;

        msg!("✅ 0.01 SOL reward sent");
        Ok(())
    }
}

// ====================== NEWS THREAD ======================
#[derive(AnchorSerialize, AnchorDeserialize, Clone, serde::Serialize, serde::Deserialize)]
pub struct NewsThread {
    pub id: String,
    pub timestamp: i64,
    pub date: String,
    pub video_date: String,
    pub title: String,
    pub content: String,
    pub post_urls: Vec<String>,
    pub post_ids: Vec<String>,
    pub video_url: String,
    pub video_tag: String,
}

// ====================== ACCOUNTS ======================
#[account]
pub struct DailyNews {
    pub posts_json: String,
    pub bump: u8,
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
    pub total_calories: u64,
    pub workouts_logged: u32,
    pub minted_phase1: bool,
    pub minted_phase2: bool,
    pub bump: u8,
}

// ====================== CONTEXTS ======================
#[derive(Accounts)]
pub struct InitializeMintConfig<'info> {
    #[account(
        init_if_needed,
        payer = authority,
        space = 8 + 32 + 1 + 4 + 4 + 4 + 200 + 1 + 1,
        seeds = [b"mint-config"],
        bump
    )]
    pub mint_config: Account<'info, MintConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateDailyNews<'info> {
    #[account(
        init_if_needed,
        payer = authority,
        space = 8 + 4 + 34000 + 1,
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
    #[account(mut, close = authority, seeds = [b"daily-news-seeker-final"], bump = daily_news.bump)]
    pub daily_news: Account<'info, DailyNews>,
    #[account(mut)]
    pub authority: Signer<'info>,
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
        space = 8 + 32 + 8 + 4 + 1 + 1 + 1,
        seeds = [b"user-state", user.key().as_ref()],
        bump
    )]
    pub user_state: Account<'info, UserState>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimDailySKR<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut, seeds = [b"mint-config"], bump = mint_config.bump)]
    pub mint_config: Account<'info, MintConfig>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 32 + 8 + 4 + 1 + 1 + 1,
        seeds = [b"user-state", user.key().as_ref()],
        bump
    )]
    pub user_state: Account<'info, UserState>,

    #[account(mut, seeds = [b"reward_vault", b"skr"], bump)]
    pub reward_vault: SystemAccount<'info>,

    #[account(
        mut,
        associated_token::mint = skr_mint,
        associated_token::authority = reward_vault
    )]
    pub reward_vault_ata: Account<'info, anchor_spl::token::TokenAccount>,

    #[account(mut)]
    pub user_token_account: Account<'info, anchor_spl::token::TokenAccount>,

    #[account(
        mut,
        address = "DCrfzg5T8hijkX8EM6oN9sh4Ucm1AMqqNZQZBGTbmofQ".parse::<Pubkey>().unwrap()
    )]
    pub skr_mint: Account<'info, anchor_spl::token::Mint>,

    pub token_program: Program<'info, anchor_spl::token::Token>,
    pub system_program: Program<'info, System>,
}



#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct ClaimDailyReward<'info> {
    #[account(mut, seeds = [b"reward_vault", exercise_id.as_bytes()], bump)]
    pub reward_vault: SystemAccount<'info>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(seeds = [b"mint-config"], bump = mint_config.bump)]
    pub mint_config: Account<'info, MintConfig>,
    pub system_program: Program<'info, System>,
}


// === ADD AFTER the last #[derive(Accounts)] block ===
#[derive(Accounts)]
pub struct AdminMintSKR<'info> {
    #[account(mut, seeds = [b"mint-config"], bump = mint_config.bump)]
    pub mint_config: Account<'info, MintConfig>,
    #[account(mut, address = "DCrfzg5T8hijkX8EM6oN9sh4Ucm1AMqqNZQZBGTbmofQ".parse::<Pubkey>().unwrap())]
    pub skr_mint: Account<'info, anchor_spl::token::Mint>,
    #[account(mut)]
    pub destination: Account<'info, anchor_spl::token::TokenAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
}

// ====================== ERRORS ======================
#[error_code]
pub enum ErrorCode {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Rewards are not enabled yet")]
    RewardsNotEnabled,
    #[msg("News JSON too large for PDA (max 34k)")]
    NewsTooLarge,
    #[msg("Invalid JSON format")]
    InvalidJson,
}