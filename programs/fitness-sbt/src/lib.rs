// TRANSACTION FORMATION PATTERN (Single Source of Truth)
// All Solana instructions in App.tsx (and any future files) must follow this exact pattern:
// Get PDAs using PublicKey.findProgramAddressSync
// Build TransactionInstruction with: keys: exactly as defined in the IDL / lib.rs
// programId: PROGRAM_ID
// data: new Uint8Array([discriminator bytes, ...optional args]) as any
// Always wrap the transaction in signAndSendRaw() → this triggers the proper Seed Vault success screen on Solana Mobile.
// No-arg instructions (e.g. toggle, claim_daily_skr, initialize) → only the 8-byte discriminator.
// Instructions with args (e.g. claim_daily_reward) → use manual length-prefixed buffer (see existing claimDailyMicroReward implementation).
// This pattern keeps everything consistent, Seeker-friendly, and easy to audit.

#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, MintTo, Token, TokenAccount, Transfer};
use anchor_spl::token;
use anchor_spl::associated_token::AssociatedToken;
use anchor_lang::system_program;





fn get_day(timestamp: i64) -> u32 {
    (timestamp / 86400) as u32
}

// Genesis NFT constant
pub const GENESIS_MINT: Pubkey = pubkey!("GT22s89nU4iWFkNXj1Bw6uYhJJWDRPpShHt4Bk8f99Te");

declare_id!("2ciBmyoCyT1f6Tu33V48kqxhu3f13qx7vXUA5se6Ydth");

// ============================================================
// MULTISIG CONFIGURATION (3-of-5) - UPGRADE AUTHORITY
// ============================================================
// Multisig Address: 2PHVFvQ6m9GLRaLiVW5FkG689C1rrUGfeEgNxSd7meoW
// Threshold: 3 / 5
// Signers:
//   1. 5fkgfLSGCxJTWcqQHfzigQUnxA1NAaCmmCjQbXmTvVzc
//   2. CwyNHESJ95mccZkGPEEApQdeB4XEV5mSL1SRkn6Ee8qG
//   3. 8TeEjQkh2CQTbKo57r3n5GrYGYUzvrmbj1eRJgbjZsjp
//   4. BpDZ6jrcPYo1GoM4DWk857ys4R7MgyZb4FmHjkC9beuH
//   5. CQsV3Wj6pdcgEkk5hkS6bd31Q2xp9fCuAqvV9WoLjqAR
// ============================================================


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
    #[msg("Exercise ID too long (max 16 bytes)")]
    InvalidExerciseId,
    #[msg("Missing Seeker Genesis NFT")]
    MissingGenesisNft,
    #[msg("Already claimed today")]
    AlreadyClaimedToday,
    #[msg("Invalid token account")]
    InvalidTokenAccount,
  
}




// === HELPER: verify Seeker Genesis NFT ownership (used by claim_daily_reward + mint_sbt) ===
fn verify_seeker_genesis_ownership(user: &Signer, user_genesis_ata: &Account<'_, TokenAccount>) -> Result<()> {
    require_keys_eq!(user_genesis_ata.mint, GENESIS_MINT, ErrorCode::MissingGenesisNft);
    require!(user_genesis_ata.amount >= 1, ErrorCode::MissingGenesisNft);
    require_keys_eq!(user_genesis_ata.owner, user.key(), ErrorCode::Unauthorized);
    Ok(())
}


#[program]
pub mod sanitas_seeker {
    use super::*;

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

  pub fn admin_mint_skr(ctx: Context<AdminMintSKR>, amount: u64) -> Result<()> {
        let config = &mut ctx.accounts.mint_config;
        require_keys_eq!(config.authority, ctx.accounts.user.key(), ErrorCode::Unauthorized);

        token::mint_to(
            CpiContext::new(
                ctx.accounts.token_program.key(),           // matches your Anchor version
                MintTo {
                    mint: ctx.accounts.skr_mint.to_account_info(),
                    to: ctx.accounts.destination.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount,
        )?;

        msg!("✅ ADMIN: Minted {} SKR", amount / 1_000_000_000);
        Ok(())
    }

    
    pub fn claim_daily_skr(ctx: Context<ClaimDailySKR>) -> Result<()> {
        // SECURITY FIX: Add Genesis NFT verification (was missing - critical, non-holders could claim)
        verify_seeker_genesis_ownership(&ctx.accounts.user, &ctx.accounts.user_genesis_ata)?;

        // SECURITY FIX: Add daily claim guard (was missing - could claim multiple times/day)
        let now = Clock::get()?.unix_timestamp;
        let now_day = get_day(now);
        let user_ex = &mut ctx.accounts.user_exercise;
        require!(user_ex.last_active_day < now_day, ErrorCode::AlreadyClaimedToday);

        let amount: u64 = 50_000_000; // 0.05 SKR

        if ctx.accounts.bootstrap_pool_token.amount >= amount {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.key(),  // matches your Anchor version
                    Transfer {
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
                    ctx.accounts.token_program.key(),  // matches your Anchor version
                    Transfer {
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

        user_ex.last_active_day = now_day;

        Ok(())
    }



    pub fn update_daily_news(ctx: Context<UpdateDailyNews>, news_json: String) -> Result<()> {
        require!(news_json.len() <= 34000, ErrorCode::NewsTooLarge);

        let _: Vec<NewsThread> = serde_json::from_str(&news_json)
            .map_err(|_| error!(ErrorCode::InvalidJson))?;

        let daily_news = &mut ctx.accounts.daily_news;
        daily_news.data = news_json.into_bytes();
        daily_news.last_updated = Clock::get()?.unix_timestamp;
        daily_news.bump = ctx.bumps.daily_news;
        daily_news.authority = ctx.accounts.user.key();

        msg!("✅ Daily news updated");
        Ok(())
    }

    pub fn reset_daily_news(ctx: Context<ResetDailyNews>) -> Result<()> {
        let news = &mut ctx.accounts.daily_news;
        require_keys_eq!(news.authority, ctx.accounts.user.key(), ErrorCode::Unauthorized);
        news.data = vec![];
        msg!("✅ Daily news PDA reset");
        Ok(())
    }

    pub fn log_workout(
        ctx: Context<LogWorkout>,
        sets: u32,
        distance_walked: u64,
        distance_ran: u64,
    ) -> Result<()> {
        verify_seeker_genesis_ownership(&ctx.accounts.user, &ctx.accounts.user_genesis_ata)?;

        let user_state = &mut ctx.accounts.user_state;
        user_state.sets_completed = user_state.sets_completed.saturating_add(sets);
        user_state.total_distance_walked = user_state.total_distance_walked.saturating_add(distance_walked);
        user_state.total_distance_ran = user_state.total_distance_ran.saturating_add(distance_ran);
        user_state.last_workout = Clock::get()?.unix_timestamp;
        user_state.bump = ctx.bumps.user_state;
        msg!("✅ Workout logged — {} sets", sets);
        Ok(())
    }

  

pub fn claim_daily_reward(
        ctx: Context<ClaimDailyReward>,
        _exercise_id: String,   // used by PDA seed — prefixed _ to silence warning
    ) -> Result<()> {
        verify_seeker_genesis_ownership(&ctx.accounts.user, &ctx.accounts.user_genesis_ata)?;

        let config = &ctx.accounts.mint_config;
        require!(config.rewards_enabled, ErrorCode::RewardsDisabled);

        let now = Clock::get()?.unix_timestamp;
        let now_day = get_day(now);
        let user_state = &mut ctx.accounts.user_state;
        require!(user_state.last_claim_day < now_day, ErrorCode::AlreadyClaimedToday);

        let transfer_amount: u64 = 10_000_000; // 0.01 SOL

        let seeds = &[
            b"reward_vault".as_ref(),
            _exercise_id.as_bytes(),
            &[ctx.bumps.reward_vault],
        ];
        let signer_seeds = &[&seeds[..]];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.system_program.key(),           // matches your Anchor version
            system_program::Transfer {
                from: ctx.accounts.reward_vault.to_account_info(),
                to: ctx.accounts.user.to_account_info(),
            },
            signer_seeds,
        );
        system_program::transfer(cpi_ctx, transfer_amount)?;

        user_state.last_claim_day = now_day;
        user_state.last_claim = now;
        msg!("✅ 0.01 SOL micro-reward sent");
        Ok(())
    }



// === FULLY UPDATED mint_sbt (Legend + 30-day + extension + labels) ===
    pub fn mint_sbt(
        ctx: Context<MintSbt>,
        _exercise_id: String,
        _video_id: String,
        label1: String,
        label2: String,
    ) -> Result<()> {
        verify_seeker_genesis_ownership(&ctx.accounts.user, &ctx.accounts.user_genesis_ata)?;

        require!(_exercise_id.len() <= 16, ErrorCode::InvalidExerciseId);
        require!(_video_id.len() <= 32, ErrorCode::InvalidExerciseId);
        require!(label1.len() <= 64, ErrorCode::InvalidExerciseId); // NEW: prevent space overflow
        require!(label2.len() <= 64, ErrorCode::InvalidExerciseId);

        let now = Clock::get()?.unix_timestamp;
        let now_day = get_day(now);

        let config = &mut ctx.accounts.exercise_config;
        let user_ex = &mut ctx.accounts.user_exercise;
        let sbt = &mut ctx.accounts.sbt;

        // Update user exercise state
        user_ex.last_active_day = now_day;
        user_ex.bump = ctx.bumps.user_exercise;

        // Check if user still has active Legend (either original or paid extension)
        let has_active_legend = user_ex.is_legend || (user_ex.extension_paid_until > now);

        // Auto-expire old Legend if >30 days inactive
        if user_ex.is_legend && (now_day - user_ex.last_active_day) > 30 {
            user_ex.is_legend = false;
        }

        // Grant Legend only if slots remain
        let is_legend_mint = has_active_legend && config.minted_legends < 1000;

        // Write SBT data
        sbt.owner = ctx.accounts.user.key();
        sbt.label1 = label1;
        sbt.label2 = label2;
        sbt.version = 1;
        sbt.is_early = is_legend_mint;
        sbt.bump = ctx.bumps.sbt;
        sbt.last_updated = now;
        sbt.data_version = 1;

        if is_legend_mint {
            config.minted_legends = config.minted_legends.saturating_add(1);
            msg!("🏆 Legend SBT minted for video {} (slot {}/{})", _video_id, config.minted_legends, 1000);
        } else {
            let amount: u64 = 1_000_000_000; // 1 SKR
            token::transfer(CpiContext::new(ctx.accounts.token_program.key(), Transfer { from: ctx.accounts.user_token_account.to_account_info(), to: ctx.accounts.stake_vault_token.to_account_info(), authority: ctx.accounts.user.to_account_info() }), amount / 2)?;
            token::transfer(CpiContext::new(ctx.accounts.token_program.key(), Transfer { from: ctx.accounts.user_token_account.to_account_info(), to: ctx.accounts.reward_pool_token.to_account_info(), authority: ctx.accounts.user.to_account_info() }), amount / 4)?;
            token::transfer(CpiContext::new(ctx.accounts.token_program.key(), Transfer { from: ctx.accounts.user_token_account.to_account_info(), to: ctx.accounts.dev_wallet.to_account_info(), authority: ctx.accounts.user.to_account_info() }), amount / 4)?;
            msg!("✅ Paid SBT minted (random frame)");
        }

        Ok(())
    }

// === NEW INSTRUCTION: Extend Legend for another 30 days (costs 1 SKR) ===
    pub fn extend_legend(
        ctx: Context<ExtendLegend>,
        _exercise_id: String,     // ← prefixed _ to kill unused warning
    ) -> Result<()> {
        // SECURITY FIX: Add Genesis NFT check (was missing for consistency)
        verify_seeker_genesis_ownership(&ctx.accounts.user, &ctx.accounts.user_genesis_ata)?;

        let user_ex = &mut ctx.accounts.user_exercise;
        let now = Clock::get()?.unix_timestamp;

        let amount: u64 = 1_000_000_000;
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.key(),  // matches your Anchor version
                Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.reward_pool_token.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount,
        )?;

        user_ex.extension_paid_until = now + (30 * 86400);
        user_ex.is_legend = true;
        user_ex.last_active_day = get_day(now);

        msg!("✅ Legend extended for 30 days on exercise {}", _exercise_id);
        Ok(())
    }



  pub fn update_fitness_stats(
        ctx: Context<UpdateFitnessStats>,
        exercise_id: String,
        walked: u64,
        ran: u64,
        sets: u32,
    ) -> Result<()> {
        require!(exercise_id.len() <= 16, ErrorCode::InvalidExerciseId);
        let sbt = &mut ctx.accounts.sbt;
        require_keys_eq!(sbt.owner, ctx.accounts.user.key(), ErrorCode::Unauthorized);
        sbt.total_distance_walked = sbt.total_distance_walked.saturating_add(walked);
        sbt.total_distance_ran = sbt.total_distance_ran.saturating_add(ran);
        sbt.total_sets_completed = sbt.total_sets_completed.saturating_add(sets);
        sbt.last_updated = Clock::get()?.unix_timestamp;
        Ok(())
    }

    pub fn update_encrypted_fitness(
        ctx: Context<UpdateEncryptedFitness>,
        _exercise_id: String,
        encrypted_data: Vec<u8>,
    ) -> Result<()> {
        let sbt = &mut ctx.accounts.sbt;
        require_keys_eq!(sbt.owner, ctx.accounts.user.key(), ErrorCode::Unauthorized);
        sbt.encrypted_fitness_data = encrypted_data;
        sbt.last_updated = Clock::get()?.unix_timestamp;
        Ok(())
    }

    pub fn update_sbt_uri(
        ctx: Context<UpdateSbtUri>,
        exercise_id: String,
        new_uri: String,
    ) -> Result<()> {
        require!(exercise_id.len() <= 16, ErrorCode::InvalidExerciseId);
        require!(new_uri.len() <= 200, ErrorCode::InvalidExerciseId); // NEW: prevent space overflow on SBT
        let sbt = &mut ctx.accounts.sbt;
        require_keys_eq!(sbt.owner, ctx.accounts.user.key(), ErrorCode::Unauthorized);
        sbt.uri = new_uri;
        Ok(())
    }

    pub fn update_sbt_descriptor(
        ctx: Context<UpdateSbtDescriptor>,
        exercise_id: String,
        new_descriptor: String,
    ) -> Result<()> {
        require!(exercise_id.len() <= 16, ErrorCode::InvalidExerciseId);
        require!(new_descriptor.len() <= 200, ErrorCode::InvalidExerciseId); // NEW: prevent space overflow
        let sbt = &mut ctx.accounts.sbt;
        require_keys_eq!(sbt.owner, ctx.accounts.user.key(), ErrorCode::Unauthorized);
        sbt.uri = new_descriptor;   // note: this updates the same field as uri (per original code)
        Ok(())
    }

// === TINY VIEW FUNCTION: Get available Legend slots for an exercise ===
    pub fn get_available_legend_slots(ctx: Context<GetAvailableLegendSlots>, _exercise_id: String) -> Result<u32> {
        let config = &ctx.accounts.exercise_config;
        let available = 1000u32.saturating_sub(config.minted_legends);
        Ok(available)
    }


    // === WITHDRAW SOL (mainnet-style recovery - only callable by upgrade authority) ===
  pub fn withdraw_sol(ctx: Context<WithdrawSol>, amount: u64) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.authority.key(),
            ctx.accounts.upgrade_authority.key(),
            ErrorCode::Unauthorized
        );

        let cpi_ctx = CpiContext::new(
            ctx.accounts.system_program.key(),   // matches your Anchor version
            system_program::Transfer {
                from: ctx.accounts.funds_account.to_account_info(),
                to: ctx.accounts.recipient.to_account_info(),
            },
        );

        system_program::transfer(cpi_ctx, amount)?;

        msg!("✅ Withdrew {} lamports from admin wallet to recipient", amount);
        Ok(())
    }



// Data structures (unchanged)
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
    pub last_claim_day: u32,
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
    pub label1: String,          // user editable (e.g. "Marathon training")
    pub label2: String,          // user editable (e.g. "Recovery walk")
    pub version: u8,
    pub is_early: bool,          // true = Legend for this video
    pub bump: u8,
    pub total_sets_completed: u32,
    pub total_distance_walked: u64,
    pub total_distance_ran: u64,
    pub encrypted_fitness_data: Vec<u8>,
    pub last_updated: i64,
    pub data_version: u8,
}

#[derive(Accounts)]
#[instruction(_exercise_id: String)]
pub struct ExtendLegend<'info> {
    #[account(mut, seeds = [b"user-exercise", user.key().as_ref(), _exercise_id.as_bytes()], bump)]
    pub user_exercise: Account<'info, UserExerciseState>,
    #[account(mut, constraint = user_token_account.mint == skr_mint.key())]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut, constraint = reward_pool_token.mint == skr_mint.key())]
    pub reward_pool_token: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub skr_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    // SECURITY FIX: Added for Genesis check
    #[account(address = GENESIS_MINT)]
    pub genesis_mint: Account<'info, Mint>,
    #[account(constraint = user_genesis_ata.mint == genesis_mint.key(), constraint = user_genesis_ata.owner == user.key())]
    pub user_genesis_ata: Account<'info, TokenAccount>,
}
 
// === NEW: Exercise-wide Legend config ===
#[account]
pub struct ExerciseConfig {
    pub exercise_id: String,
    pub minted_legends: u32,
    pub last_video_id: String,
    pub last_reset_day: u32,
    pub bump: u8,
}

// === NEW: Per-user + per-exercise state ===
#[account]
pub struct UserExerciseState {
    pub sets_completed: u32,
    pub total_distance_walked: u64,
    pub total_distance_ran: u64,
    pub last_active_day: u32,
    pub extension_paid_until: i64,
    pub is_legend: bool,
    pub bump: u8,
}

#[derive(Accounts)]
#[instruction(_exercise_id: String)]
pub struct GetAvailableLegendSlots<'info> {
    #[account(seeds = [b"exercise-config", _exercise_id.as_bytes()], bump)]
    pub exercise_config: Account<'info, ExerciseConfig>,
}


#[derive(Accounts)]
pub struct WithdrawSol<'info> {
    #[account(mut)]
    pub funds_account: Signer<'info>,                    // Admin wallet holding SOL for recovery
    #[account(mut)]
    pub recipient: SystemAccount<'info>,                 // e.g. B9Qo wallet
    pub authority: Signer<'info>,                        // signer (must match upgrade auth)
    /// CHECK: Upgrade authority (will be multisig)
    pub upgrade_authority: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}




// Account contexts (bump syntax fixed for Anchor compatibility)
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
    #[account(init_if_needed, payer = user, space = 8 + 32 + 4 + 34000 + 8 + 1, seeds = [b"daily-news-seeker-final"], bump)]
    pub daily_news: Account<'info, DailyNews>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResetDailyNews<'info> {
    #[account(mut, seeds = [b"daily-news-seeker-final"], bump = daily_news.bump)]
    pub daily_news: Account<'info, DailyNews>,
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
    #[account(mut, address = pubkey!("DCrfzg5T8hijkX8EM6oN9sh4Ucm1AMqqNZQZBGTbmofQ"))]
    pub skr_mint: Account<'info, Mint>,
    #[account(mut)]
    pub destination: Account<'info, TokenAccount>,
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct LogWorkout<'info> {
    #[account(init_if_needed, payer = user, space = 8 + 4 + 8 + 8 + 8 + 4 + 4 + 1, seeds = [b"user-state", user.key().as_ref()], bump)]
    pub user_state: Account<'info, UserState>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(address = GENESIS_MINT)]
    pub genesis_mint: Account<'info, Mint>,
    #[account(constraint = user_genesis_ata.mint == genesis_mint.key(), constraint = user_genesis_ata.owner == user.key())]
    pub user_genesis_ata: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimDailySKR<'info> {
    #[account(mut, seeds = [b"bootstrap-pool"], bump)]
    pub bootstrap_pool: Account<'info, BootstrapPool>,
    #[account(mut, associated_token::mint = skr_mint, associated_token::authority = bootstrap_pool)]
    pub bootstrap_pool_token: Box<Account<'info, TokenAccount>>,
    #[account(mut, seeds = [b"reward-pool"], bump)]
    pub reward_pool: Account<'info, RewardPool>,
    #[account(mut, associated_token::mint = skr_mint, associated_token::authority = reward_pool)]
    pub reward_pool_token: Box<Account<'info, TokenAccount>>,
    #[account(mut, seeds = [b"user-exercise", user.key().as_ref(), b"sprint-interval".as_ref()], bump)]
    pub user_exercise: Box<Account<'info, UserExerciseState>>,
    #[account(mut, associated_token::mint = skr_mint, associated_token::authority = user)]
    pub user_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub skr_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    // SECURITY FIX: Added Genesis accounts (was missing)
    #[account(address = GENESIS_MINT)]
    pub genesis_mint: Account<'info, Mint>,
    #[account(constraint = user_genesis_ata.mint == genesis_mint.key(), constraint = user_genesis_ata.owner == user.key())]
    pub user_genesis_ata: Box<Account<'info, TokenAccount>>,
}



#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct ClaimDailyReward<'info> {
    /// CHECK: This is a PDA-owned reward vault that signs the SOL transfer.
    ///        Seeds are verified inside the instruction + bump is provided.
    ///        No additional type checks needed — this is the canonical reward vault.
    #[account(mut, seeds = [b"reward_vault", exercise_id.as_bytes()], bump)]
    pub reward_vault: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [b"mint-config"], bump)]
    pub mint_config: Account<'info, MintConfig>,
    #[account(mut, seeds = [b"user-state", user.key().as_ref()], bump)]
    pub user_state: Account<'info, UserState>,
    #[account(address = GENESIS_MINT)]
    pub genesis_mint: Account<'info, Mint>,
    #[account(constraint = user_genesis_ata.mint == genesis_mint.key(), constraint = user_genesis_ata.owner == user.key())]
    pub user_genesis_ata: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
}



#[derive(Accounts)]
#[instruction(_exercise_id: String, _video_id: String)]
pub struct MintSbt<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    
    // Core PDAs (Box to save stack space)
    #[account(mut, seeds = [b"counter"], bump)]
    pub counter: Box<Account<'info, Counter>>,
    
    #[account(
        init_if_needed, 
        payer = user, 
        space = 8 + 32 + 4 + 200 + 4 + 64 + 4 + 64 + 1 + 1 + 1 + 4 + 8 + 8 + 8 + 4 + 8 + 1, 
        seeds = [user.key().as_ref(), b"user-sbt", _exercise_id.as_bytes()], 
        bump
    )]
    pub sbt: Box<Account<'info, SbtAccount>>,
    
    // Grouped exercise state (Box all large accounts)
    #[account(init_if_needed, payer = user, space = 8 + 4 + 8 + 8 + 8 + 4 + 8 + 1, seeds = [b"user-exercise", user.key().as_ref(), _exercise_id.as_bytes()], bump)]
    pub user_exercise: Box<Account<'info, UserExerciseState>>,
    
    #[account(init_if_needed, payer = user, space = 8 + 16 + 4 + 32 + 4 + 1, seeds = [b"exercise-config", _exercise_id.as_bytes()], bump)]
    pub exercise_config: Box<Account<'info, ExerciseConfig>>,
    
    // Token accounts (Box to save stack)
    #[account(mut, constraint = user_token_account.mint == skr_mint.key())]
    pub user_token_account: Box<Account<'info, TokenAccount>>,
    
    #[account(mut, constraint = reward_pool_token.mint == skr_mint.key())]
    pub reward_pool_token: Box<Account<'info, TokenAccount>>,
    
    #[account(init_if_needed, payer = user, associated_token::mint = skr_mint, associated_token::authority = stake_vault)]
    pub stake_vault_token: Box<Account<'info, TokenAccount>>,
    
    // Genesis NFT verification
    #[account(address = GENESIS_MINT)]
    pub genesis_mint: Box<Account<'info, Mint>>,
    
    #[account(constraint = user_genesis_ata.mint == genesis_mint.key(), constraint = user_genesis_ata.owner == user.key())]
    pub user_genesis_ata: Box<Account<'info, TokenAccount>>,
    
    // Vaults & External
    /// CHECK: PDA-owned stake vault
    #[account(seeds = [b"stake-vault"], bump)]
    pub stake_vault: UncheckedAccount<'info>,
    
    #[account(mut, address = pubkey!("B9Qo6q398kvryKQuCUMjRxQHMbVTGTc3wwSbrRoKaTrc"))]
    pub dev_wallet: Box<Account<'info, TokenAccount>>,
    
    // Shared accounts (no Box needed for small/readonly)
    pub skr_mint: Account<'info, Mint>,
    
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}



#[derive(Accounts)]
pub struct DepositToRewardPool<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut, seeds = [b"reward-pool"], bump)]
    pub reward_pool: Account<'info, RewardPool>,
    #[account(mut, constraint = reward_pool_token.mint == skr_mint.key())]
    pub reward_pool_token: Account<'info, TokenAccount>,
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
    #[account(mut, constraint = bootstrap_pool_token.mint == skr_mint.key())]
    pub bootstrap_pool_token: Account<'info, TokenAccount>,
    pub skr_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct UpdateFitnessStats<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [user.key().as_ref(), b"user-sbt", exercise_id.as_bytes()], bump)]
    pub sbt: Account<'info, SbtAccount>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct UpdateEncryptedFitness<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [user.key().as_ref(), b"user-sbt", exercise_id.as_bytes()], bump)]
    pub sbt: Account<'info, SbtAccount>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct UpdateSbtUri<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [user.key().as_ref(), b"user-sbt", exercise_id.as_bytes()], bump)]
    pub sbt: Account<'info, SbtAccount>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct UpdateSbtDescriptor<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [user.key().as_ref(), b"user-sbt", exercise_id.as_bytes()], bump)]
    pub sbt: Account<'info, SbtAccount>,
}



}