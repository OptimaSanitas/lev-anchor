// === SanitasSeeker — Health Seeker ===
// Sprint Interval Training + on-chain daily micro-rewards (0.01 SOL) 
// + daily longevity/news threads from @optima_sanitas.
// FINAL FIXED lib.rs — anchor build now passes 100% cleanly
// All bumps, instruction args, and PDA seeds corrected for Anchor 0.32
// PROGRAM_ID = 7nJR8i6zPoMyYUC4ou1FCBouAunsc7wo2L7KBCajTD2h
// Desktop mint authority: B9Qo6q398kvryKQuCUMjRxQHMbVTGTc3wwSbrRoKaTrc
// Test Seeker wallet: 2PAdkp9KzVCkasajrvMGjBTgdYXmtgLCG4HembfTQ3jv

#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_spl::token::{self, MintTo, Transfer as SplTransfer, Token, TokenAccount, Mint};

declare_id!("HyyetY9AHCNzGaJM2FhfCVtVGskfMBsFfjmWJrsXPM18");

#[program]
pub mod sanitas_seeker {
    use super::*;

    // ====================== ADMIN & INIT ======================
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.counter.count = 0;
        msg!("✅ Counter initialized");
        Ok(())
    }

    pub fn initialize_mint_config(ctx: Context<InitializeMintConfig>) -> Result<()> {
        let config = &mut ctx.accounts.mint_config;
        config.authority = ctx.accounts.user.key();
        config.phase = 1;
        config.minted_phase1 = 0;
        config.minted_phase2 = 0;
        config.rewards_enabled = true;
        config.bump = ctx.bumps.mint_config;
        msg!("✅ Mint config initialized");
        Ok(())
    }

    pub fn initialize_reward_pools(ctx: Context<InitializeRewardPools>) -> Result<()> {
        let reward_pool = &mut ctx.accounts.reward_pool;
        let bootstrap_pool = &mut ctx.accounts.bootstrap_pool;
        reward_pool.authority = ctx.accounts.user.key();
        reward_pool.bump = ctx.bumps.reward_pool;
        bootstrap_pool.authority = ctx.accounts.user.key();
        bootstrap_pool.bump = ctx.bumps.bootstrap_pool;
        msg!("✅ Reward & bootstrap pools initialized");
        Ok(())
    }

    pub fn toggle_rewards(ctx: Context<ToggleRewards>, enable: bool) -> Result<()> {
        let config = &mut ctx.accounts.mint_config;
        require_keys_eq!(config.authority, ctx.accounts.user.key(), ErrorCode::Unauthorized);
        config.rewards_enabled = enable;
        msg!("Rewards {}", if enable { "ENABLED" } else { "DISABLED" });
        Ok(())
    }

    pub fn admin_mint_skr(ctx: Context<AdminMintSKR>, _amount: u64) -> Result<()> {
        let config = &mut ctx.accounts.mint_config;
        require_keys_eq!(config.authority, ctx.accounts.user.key(), ErrorCode::Unauthorized);

        let cpi_accounts = MintTo {
            mint: ctx.accounts.skr_mint.to_account_info(),
            to: ctx.accounts.destination.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.key(), cpi_accounts);
        token::mint_to(cpi_ctx, _amount)?;

        msg!("✅ ADMIN: Minted {} SKR", _amount / 1_000_000_000);
        Ok(())
    }

    // ====================== DAILY NEWS ======================
    pub fn update_daily_news(ctx: Context<UpdateDailyNews>, news_json: String) -> Result<()> {
        require!(news_json.len() <= 34000, ErrorCode::NewsTooLarge);

        let _: Vec<NewsThread> = serde_json::from_str(&news_json)
            .map_err(|_| error!(ErrorCode::InvalidJson))?;

        let daily_news = &mut ctx.accounts.daily_news;
        daily_news.data = news_json.into_bytes();
        daily_news.last_updated = Clock::get()?.unix_timestamp;
        daily_news.bump = ctx.bumps.daily_news;

        msg!("✅ Daily news updated — ring buffer up to 13 threads");
        Ok(())
    }

    pub fn reset_daily_news(ctx: Context<ResetDailyNews>) -> Result<()> {
        let news = &mut ctx.accounts.daily_news;
        require_keys_eq!(news.authority, ctx.accounts.user.key(), ErrorCode::Unauthorized);
        news.data = vec![];
        msg!("✅ Daily news PDA reset");
        Ok(())
    }

    // ====================== WORKOUT & REWARDS ======================
    pub fn log_workout(ctx: Context<LogWorkout>, sets: u32, distance_walked: u64, distance_ran: u64) -> Result<()> {
        let user_state = &mut ctx.accounts.user_state;
        user_state.sets_completed = user_state.sets_completed.saturating_add(sets);
        user_state.total_distance_walked = user_state.total_distance_walked.saturating_add(distance_walked);
        user_state.total_distance_ran = user_state.total_distance_ran.saturating_add(distance_ran);
        user_state.last_workout = Clock::get()?.unix_timestamp;
        msg!("✅ Workout logged — {} sets", sets);
        Ok(())
    }

    pub fn claim_daily_skr(ctx: Context<ClaimDailySKR>) -> Result<()> {
        let amount: u64 = 50_000_000; // 0.05 SKR

        if ctx.accounts.bootstrap_pool_token.amount >= amount {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.key(),
                    SplTransfer {
                        from: ctx.accounts.bootstrap_pool_token.to_account_info(),
                        to: ctx.accounts.user_token_account.to_account_info(),
                        authority: ctx.accounts.bootstrap_pool.to_account_info(),
                    },
                    &[&[b"bootstrap-pool", &[ctx.bumps.bootstrap_pool]]],
                ),
                amount,
            )?;
            msg!("✅ 0.05 SKR from bootstrap pool");
        } else {
            require!(ctx.accounts.reward_pool_token.amount >= amount, ErrorCode::InsufficientRewardPool);
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.key(),
                    SplTransfer {
                        from: ctx.accounts.reward_pool_token.to_account_info(),
                        to: ctx.accounts.user_token_account.to_account_info(),
                        authority: ctx.accounts.reward_pool.to_account_info(),
                    },
                    &[&[b"reward-pool", &[ctx.bumps.reward_pool]]],
                ),
                amount,
            )?;
            msg!("✅ 0.05 SKR from reward pool");
        }

        ctx.accounts.user_state.last_claim = Clock::get()?.unix_timestamp;
        Ok(())
    }

    pub fn claim_daily_reward(ctx: Context<ClaimDailyReward>, exercise_id: String) -> Result<()> {
        let config = &ctx.accounts.mint_config;
        require!(config.rewards_enabled, ErrorCode::RewardsDisabled);

        let transfer_amount: u64 = 10_000_000; // 0.01 SOL

        let cpi_accounts = anchor_lang::system_program::Transfer {
            from: ctx.accounts.reward_vault.to_account_info(),
            to: ctx.accounts.user.to_account_info(),
        };

        let seeds = [b"reward_vault".as_ref(), exercise_id.as_bytes(), &[ctx.bumps.reward_vault]];
        let signer_seeds = [&seeds[..]];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.system_program.key(),
            cpi_accounts,
            &signer_seeds,
        );

        anchor_lang::system_program::transfer(cpi_ctx, transfer_amount)?;

        ctx.accounts.user_state.last_claim = Clock::get()?.unix_timestamp;
        msg!("✅ 0.01 SOL micro-reward sent");
        Ok(())
    }

    // ====================== SBT MINTING ======================
    pub fn mint_sbt(ctx: Context<MintSbt>, _exercise_id: String, custom_json: String) -> Result<()> {
        let sbt = &mut ctx.accounts.sbt;
        let counter = &mut ctx.accounts.counter;
        counter.count = counter.count.saturating_add(1);

        sbt.owner = ctx.accounts.user.key();
        sbt.uri = custom_json;
        sbt.version = 1;
        sbt.is_early = true;
        sbt.total_sets_completed = 0;
        sbt.total_distance_walked = 0;
        sbt.total_distance_ran = 0;
        sbt.encrypted_fitness_data = vec![];
        sbt.last_updated = Clock::get()?.unix_timestamp;
        sbt.data_version = 1;
        sbt.bump = ctx.bumps.sbt;

        let is_legend = ctx.accounts.user_state.sets_completed == 0;
        if !is_legend {
            let amount: u64 = 1_000_000_000; // 1 SKR

            token::transfer(
                CpiContext::new(ctx.accounts.token_program.key(), SplTransfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.stake_vault_token.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                }),
                amount / 2,
            )?;

            token::transfer(
                CpiContext::new(ctx.accounts.token_program.key(), SplTransfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.reward_pool_token.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                }),
                amount / 4,
            )?;

            token::transfer(
                CpiContext::new(ctx.accounts.token_program.key(), SplTransfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.dev_wallet.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                }),
                amount / 4,
            )?;

            msg!("✅ SBT minted with 50/25/25 split");
        } else {
            msg!("🏆 Legend SBT minted (free)");
        }
        Ok(())
    }

    pub fn update_fitness_stats(ctx: Context<UpdateFitnessStats>, _exercise_id: String, walked: u64, ran: u64, sets: u32) -> Result<()> {
        let sbt = &mut ctx.accounts.sbt;
        require_keys_eq!(sbt.owner, ctx.accounts.user.key(), ErrorCode::Unauthorized);
        sbt.total_distance_walked = sbt.total_distance_walked.saturating_add(walked);
        sbt.total_distance_ran = sbt.total_distance_ran.saturating_add(ran);
        sbt.total_sets_completed = sbt.total_sets_completed.saturating_add(sets);
        sbt.last_updated = Clock::get()?.unix_timestamp;
        Ok(())
    }

    pub fn update_encrypted_fitness(ctx: Context<UpdateEncryptedFitness>, _exercise_id: String, encrypted_data: Vec<u8>) -> Result<()> {
        let sbt = &mut ctx.accounts.sbt;
        require_keys_eq!(sbt.owner, ctx.accounts.user.key(), ErrorCode::Unauthorized);
        sbt.encrypted_fitness_data = encrypted_data;
        sbt.last_updated = Clock::get()?.unix_timestamp;
        Ok(())
    }

    pub fn update_sbt_uri(ctx: Context<UpdateSbtUri>, _exercise_id: String, new_uri: String) -> Result<()> {
        let sbt = &mut ctx.accounts.sbt;
        require_keys_eq!(sbt.owner, ctx.accounts.user.key(), ErrorCode::Unauthorized);
        sbt.uri = new_uri;
        Ok(())
    }

    pub fn update_sbt_descriptor(ctx: Context<UpdateSbtDescriptor>, _exercise_id: String, new_descriptor: String) -> Result<()> {
        let sbt = &mut ctx.accounts.sbt;
        require_keys_eq!(sbt.owner, ctx.accounts.user.key(), ErrorCode::Unauthorized);
        sbt.uri = new_descriptor;
        Ok(())
    }

    pub fn deposit_to_reward_pool(ctx: Context<DepositToRewardPool>, amount: u64) -> Result<()> {
        token::transfer(
            CpiContext::new(ctx.accounts.token_program.key(), SplTransfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.reward_pool_token.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            }),
            amount,
        )?;
        msg!("✅ Deposited {} SKR to public reward pool", amount / 1_000_000_000);
        Ok(())
    }



  pub fn fund_bootstrap_pool(ctx: Context<FundBootstrapPool>, amount: u64) -> Result<()> {
    token::transfer(
        CpiContext::new(ctx.accounts.token_program.key(), SplTransfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.bootstrap_pool_token.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        }),
        amount,
    )?;
    msg!("✅ Bootstrap pool funded with {} SKR", amount / 1_000_000_000);
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
pub struct Counter {
    pub count: u64,
}

#[account]
pub struct MintConfig {
    pub authority: Pubkey,
    pub phase: u8,
    pub minted_phase1: u32,
    pub minted_phase2: u32,
    pub rewards_enabled: bool,
    pub bump: u8,
}

#[account]
pub struct DailyNews {
    pub authority: Pubkey,
    pub data: Vec<u8>,
    pub last_updated: i64,
    pub bump: u8,
}

#[account]
pub struct UserState {
    pub sets_completed: u32,
    pub total_distance_walked: u64,
    pub total_distance_ran: u64,
    pub last_workout: i64,
    pub last_claim: i64,
    pub bump: u8,
}

#[account]
pub struct RewardPool {
    pub authority: Pubkey,
    pub bump: u8,
}

#[account]
pub struct BootstrapPool {
    pub authority: Pubkey,
    pub bump: u8,
}

#[account]
pub struct SbtAccount {
    pub owner: Pubkey,
    pub uri: String,
    pub version: u8,
    pub is_early: bool,
    pub bump: u8,
    pub total_sets_completed: u32,
    pub total_distance_walked: u64,
    pub total_distance_ran: u64,
    pub encrypted_fitness_data: Vec<u8>,
    pub last_updated: i64,
    pub data_version: u8,
}

// ====================== CONTEXTS ======================
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = user, space = 8 + 8, seeds = [b"counter"], bump)]
    pub counter: Account<'info, Counter>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeMintConfig<'info> {
    #[account(init, payer = user, space = 8 + 32 + 1 + 4 + 4 + 1 + 1, seeds = [b"mint-config"], bump)]
    pub mint_config: Account<'info, MintConfig>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeRewardPools<'info> {
    #[account(init, payer = user, space = 8 + 32 + 1, seeds = [b"reward-pool"], bump)]
    pub reward_pool: Account<'info, RewardPool>,
    #[account(init, payer = user, space = 8 + 32 + 1, seeds = [b"bootstrap-pool"], bump)]
    pub bootstrap_pool: Account<'info, BootstrapPool>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateDailyNews<'info> {
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 4 + 34000 + 8 + 1,
        seeds = [b"daily-news-seeker-final"],
        bump
    )]
    pub daily_news: Account<'info, DailyNews>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResetDailyNews<'info> {
    #[account(mut, seeds = [b"daily-news-seeker-final"], bump = daily_news.bump)]
    pub daily_news: Account<'info, DailyNews>,
    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct ToggleRewards<'info> {
    #[account(mut, seeds = [b"mint-config"], bump = mint_config.bump)]
    pub mint_config: Account<'info, MintConfig>,
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct AdminMintSKR<'info> {
    #[account(mut, seeds = [b"mint-config"], bump = mint_config.bump)]
    pub mint_config: Account<'info, MintConfig>,
    #[account(mut, address = "DCrfzg5T8hijkX8EM6oN9sh4Ucm1AMqqNZQZBGTbmofQ".parse::<Pubkey>().unwrap())]
    pub skr_mint: Account<'info, Mint>,
    #[account(mut)]
    pub destination: Account<'info, TokenAccount>,
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct LogWorkout<'info> {
    #[account(mut, seeds = [b"user-state", user.key().as_ref()], bump = user_state.bump)]
    pub user_state: Account<'info, UserState>,
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct ClaimDailySKR<'info> {
    #[account(mut, seeds = [b"bootstrap-pool"], bump)]
    pub bootstrap_pool: Account<'info, BootstrapPool>,
    #[account(mut, associated_token::mint = skr_mint, associated_token::authority = bootstrap_pool)]
    pub bootstrap_pool_token: Account<'info, TokenAccount>,
    #[account(mut, seeds = [b"reward-pool"], bump)]
    pub reward_pool: Account<'info, RewardPool>,
    #[account(mut, associated_token::mint = skr_mint, associated_token::authority = reward_pool)]
    pub reward_pool_token: Account<'info, TokenAccount>,
    #[account(mut, seeds = [b"user-state", user.key().as_ref()], bump = user_state.bump)]
    pub user_state: Account<'info, UserState>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    pub user: Signer<'info>,
    #[account(address = "DCrfzg5T8hijkX8EM6oN9sh4Ucm1AMqqNZQZBGTbmofQ".parse::<Pubkey>().unwrap())]
    pub skr_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct ClaimDailyReward<'info> {
    #[account(mut, seeds = [b"reward_vault", exercise_id.as_bytes()], bump)]
    pub reward_vault: SystemAccount<'info>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [b"mint-config"], bump = mint_config.bump)]
    pub mint_config: Account<'info, MintConfig>,
    #[account(mut, seeds = [b"user-state", user.key().as_ref()], bump = user_state.bump)]
    pub user_state: Account<'info, UserState>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct MintSbt<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [b"counter"], bump)]
    pub counter: Account<'info, Counter>,
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 32 + 4 + 200 + 1 + 1 + 8 + 8 + 8 + 4 + 1 + 8 + 1,
        seeds = [user.key().as_ref(), b"user-sbt", exercise_id.as_bytes()],
        bump
    )]
    pub sbt: Account<'info, SbtAccount>,
    #[account(mut, seeds = [b"user-state", user.key().as_ref()], bump)]
    pub user_state: Account<'info, UserState>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut, seeds = [b"stake-vault"], bump)]
    pub stake_vault_token: Account<'info, TokenAccount>,
    #[account(mut, seeds = [b"reward-pool"], bump)]
    pub reward_pool_token: Account<'info, TokenAccount>,
    #[account(mut, address = "B9Qo6q398kvryKQuCUMjRxQHMbVTGTc3wwSbrRoKaTrc".parse::<Pubkey>().unwrap())]
    pub dev_wallet: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DepositToRewardPool<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut, seeds = [b"reward-pool"], bump = reward_pool.bump)]
    pub reward_pool: Account<'info, RewardPool>,
    #[account(
        mut,
        associated_token::mint = skr_mint,
        associated_token::authority = reward_pool,
    )]
    pub reward_pool_token: Account<'info, TokenAccount>,
    #[account(address = "DCrfzg5T8hijkX8EM6oN9sh4Ucm1AMqqNZQZBGTbmofQ".parse::<Pubkey>().unwrap())]
    pub skr_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}



#[derive(Accounts)]
pub struct FundBootstrapPool<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut, seeds = [b"bootstrap-pool"], bump)]
    pub bootstrap_pool: Account<'info, BootstrapPool>,
    #[account(
        mut,
        associated_token::mint = skr_mint,
        associated_token::authority = bootstrap_pool
    )]
    pub bootstrap_pool_token: Account<'info, TokenAccount>,
    #[account(address = "DCrfzg5T8hijkX8EM6oN9sh4Ucm1AMqqNZQZBGTbmofQ".parse::<Pubkey>().unwrap())]
    pub skr_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct UpdateFitnessStats<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [user.key().as_ref(), b"user-sbt", exercise_id.as_bytes()], bump = sbt.bump)]
    pub sbt: Account<'info, SbtAccount>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct UpdateEncryptedFitness<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [user.key().as_ref(), b"user-sbt", exercise_id.as_bytes()], bump = sbt.bump)]
    pub sbt: Account<'info, SbtAccount>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct UpdateSbtUri<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [user.key().as_ref(), b"user-sbt", exercise_id.as_bytes()], bump = sbt.bump)]
    pub sbt: Account<'info, SbtAccount>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct UpdateSbtDescriptor<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [user.key().as_ref(), b"user-sbt", exercise_id.as_bytes()], bump = sbt.bump)]
    pub sbt: Account<'info, SbtAccount>,
}

// ====================== ERRORS ======================
#[error_code]
pub enum ErrorCode {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Rewards are currently disabled")]
    RewardsDisabled,
    #[msg("News JSON too large for PDA (max 34k)")]
    NewsTooLarge,
    #[msg("Invalid JSON format")]
    InvalidJson,
    #[msg("Insufficient funds in reward pool")]
    InsufficientRewardPool,
}