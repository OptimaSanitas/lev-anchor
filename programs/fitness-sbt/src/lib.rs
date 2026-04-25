// ============================================================
// Sanitas Seeker — BETA-MINIMAL program (SKE-Claim + pool + news + log)
// Full-feature source is archived: fitness-sbt/archive/sanitas_seeker_lib_full_2026-04-24.rs
// Upgrade: deploy/upgrade with same `declare_id!` only if you intend to replace the binary; keep archive for restore.
//
// Roadmap (not in this binary): frame cNFT vs legend instrument — see docs/CNFT_LEGEND_ROADMAP.md
// ============================================================
#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
// SKR uses legacy Token program; Seeker Genesis mint/ATA are Token-2022 — use `token_interface` for those.
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, MintTo, Token, TokenAccount, Transfer};
use anchor_spl::token_interface::{Mint as GenesisMintTy, TokenAccount as GenesisTokenAccount};
use anchor_spl::token;

/// Seconds added to Unix time before /86400, so the "game day" rolls at a US-West–biased boundary for Seeker users.
/// 8h = crude PST alignment (no DST). Set to `0` for pure UTC days.
const CLAIM_DAY_OFFSET_SEC: i64 = 8 * 3_600;

fn claim_day(ts: i64) -> u32 {
    (ts.saturating_add(CLAIM_DAY_OFFSET_SEC) / 86400) as u32
}

/// Global cap on **legend instruments** (separate from frame cNFTs / claim cohort). See `docs/CNFT_LEGEND_ROADMAP.md`.
pub const LEGEND_INSTRUMENT_CAP: u32 = 10_000;

/// Default floor (raw SKR lamports, 9 decimals): **1_000_000 SKR** — only **balance above** this in the reward pool is splittable.
pub const POOL_SPLIT_THRESHOLD_SKR_RAW_DEFAULT: u64 = 1_000_000_000_000_000;

/// 100% in basis points (`distribute_bps` / `BPS_DENOM`).
pub const BPS_DENOM: u16 = 10_000;

// --- Pool economics (this program) ---
// SKR is a third-party (Solana Mobile Seeker ecosystem) SPL mint—integrated here, not issued by this program.
// Reward pool + bootstrap pool are plain SPL token accounts: SKR sits idle until a transfer instruction runs.
// Daily claims debit bootstrap first, then reward pool (see `claim_daily_skr`). No CPI to Solana Mobile’s SKR
// staking or other yield venues. Post-threshold `split_reward_pool_excess` moves excess to stake-vault /
// bootstrap / treasury ATAs as custody buckets; treasury/ops may stake that SKR elsewhere using keys they control.

pub const GENESIS_MINT: Pubkey = pubkey!("GT22s89nU4iWFkNXj1Bw6uYhJJWDRPpShHt4Bk8f99Te");

declare_id!("AwZRzJmcbRx3weqFXUi3MWhaEsS6a7GjvkCJH2DUTkhN");

#[error_code]
pub enum ErrorCode {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Rewards are currently disabled")]
    RewardsDisabled,
    #[msg("News JSON too large for PDA (max 34k)")]
    NewsTooLarge,
    #[msg("Insufficient funds in reward pool")]
    InsufficientRewardPool,
    #[msg("Invalid token account")]
    InvalidTokenAccount,
    #[msg("Already claimed today for this day bucket")]
    AlreadyClaimedToday,
    #[msg("Missing Seeker Genesis NFT")]
    MissingGenesisNft,
    #[msg("Complete at least one logged sprint set before claim")]
    NeedOneSetForClaim,
    #[msg("Legend instrument global cap reached (10_000)")]
    LegendSupplyCapReached,
    #[msg("Invalid legend mint count")]
    InvalidLegendMintCount,
    #[msg("Reward pool balance is not above the configured split threshold")]
    BelowSplitThreshold,
    #[msg("Treasury token account owner does not match pool split config")]
    TreasuryMismatch,
    #[msg("Arithmetic overflow in split calculation")]
    SplitMathOverflow,
    #[msg("Treasury policy change is still inside the configured timelock window")]
    TreasuryPolicyTimelockActive,
    #[msg("Invalid treasury policy parameter (e.g. bps > 10_000)")]
    InvalidTreasuryPolicy,
    #[msg("Exercise / label string too long")]
    InvalidExerciseId,
    #[msg("SBT already exists for this exercise")]
    SbtAlreadyMinted,
    #[msg("Insufficient SKR in user account for paid mint")]
    InsufficientUserSkr,
    #[msg("Timestamp arithmetic overflow")]
    TimestampOverflow,
    #[msg("Exercise id does not match legend entitlement")]
    LegendExerciseMismatch,
    #[msg("Legend sale is already listed")]
    LegendSaleAlreadyListed,
    #[msg("Legend sale is not listed")]
    LegendSaleNotListed,
    #[msg("Legend sale price must be > 0")]
    LegendSaleInvalidPrice,
}

/// `legend_sale.state`
pub const LEGEND_SALE_VACANT: u8 = 0;
pub const LEGEND_SALE_LISTED: u8 = 1;
pub const LEGEND_SALE_CANCELLED: u8 = 3;

/// Genesis passes through `token_interface` so the ATA may be **spl-token** or **Token-2022** (Seeker SGT).
/// Anchor enforces the account is owned by an approved token program, not a fake data account.
/// Enforces `min_change_interval_sec` between **updates** after the first one (`last_change_ts != 0`).
fn assert_treasury_policy_timelock(last_change_ts: i64, min_change_interval_sec: u64) -> Result<()> {
    if min_change_interval_sec == 0 || last_change_ts == 0 {
        return Ok(());
    }
    let now = Clock::get()?.unix_timestamp;
    let min_i = i64::try_from(min_change_interval_sec).map_err(|_| error!(ErrorCode::InvalidTreasuryPolicy))?;
    let next = last_change_ts
        .checked_add(min_i)
        .ok_or(error!(ErrorCode::InvalidTreasuryPolicy))?;
    require!(now >= next, ErrorCode::TreasuryPolicyTimelockActive);
    Ok(())
}

fn verify_seeker_genesis_ownership(
    expected_mint: &Pubkey,
    user: &Signer,
    user_genesis_ata: &InterfaceAccount<'_, GenesisTokenAccount>,
) -> Result<()> {
    require_keys_eq!(user_genesis_ata.mint, *expected_mint, ErrorCode::MissingGenesisNft);
    require!(user_genesis_ata.amount >= 1, ErrorCode::MissingGenesisNft);
    require_keys_eq!(user_genesis_ata.owner, user.key(), ErrorCode::Unauthorized);
    Ok(())
}

#[program]
pub mod sanitas_seeker {
    use super::*;

    // ==================== INITIALIZATION ====================
    pub fn initialize_mint_config(ctx: Context<InitializeMintConfig>) -> Result<()> {
        let c = &mut ctx.accounts.mint_config;
        c.authority = ctx.accounts.user.key();
        c.phase = 1;
        c.minted_phase1 = 0;
        c.minted_phase2 = 0;
        c.rewards_enabled = true;
        c.bump = ctx.bumps.mint_config;
        msg!("min mint config ok");
        Ok(())
    }

    pub fn initialize_reward_pools(ctx: Context<InitializeRewardPools>) -> Result<()> {
        let r = &mut ctx.accounts.reward_pool;
        r.authority = ctx.accounts.user.key();
        r.bump = ctx.bumps.reward_pool;
        let b = &mut ctx.accounts.bootstrap_pool;
        b.authority = ctx.accounts.user.key();
        b.bump = ctx.bumps.bootstrap_pool;
        msg!("pools ok");
        Ok(())
    }

    // ==================== ADMIN ====================
    pub fn toggle_rewards(ctx: Context<ToggleRewards>, enable: bool) -> Result<()> {
        let c = &mut ctx.accounts.mint_config;
        require_keys_eq!(c.authority, ctx.accounts.user.key(), ErrorCode::Unauthorized);
        c.rewards_enabled = enable;
        Ok(())
    }

    pub fn admin_mint_skr(ctx: Context<AdminMintSKR>, amount: u64) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.mint_config.authority,
            ctx.accounts.user.key(),
            ErrorCode::Unauthorized
        );
        token::mint_to(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                MintTo {
                    mint: ctx.accounts.skr_mint.to_account_info(),
                    to: ctx.accounts.destination.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount,
        )?;
        Ok(())
    }

    // ==================== USER ====================
    /// **0.05 SKR** per successful claim, **once per game-day** bucket (`claim_day`).
    /// **Order of spend:** `bootstrap_pool_token` is debited first; if its balance is below `AM`, `reward_pool_token`
    /// pays the shortfall. SKR does not earn yield inside these accounts—only SPL transfers execute here.
    pub fn claim_daily_skr(ctx: Context<ClaimDailySKR>) -> Result<()> {
        let cfg = &mut ctx.accounts.mint_config;
        require!(cfg.rewards_enabled, ErrorCode::RewardsDisabled);
        verify_seeker_genesis_ownership(
            &ctx.accounts.genesis_gate.seeker_genesis_mint,
            &ctx.accounts.user,
            &ctx.accounts.user_genesis_ata,
        )?;
        require!(
            ctx.accounts.user_state.sets_completed >= 1,
            ErrorCode::NeedOneSetForClaim
        );
        let now = Clock::get()?.unix_timestamp;
        let now_day = claim_day(now);
        let ux = &mut ctx.accounts.user_exercise;
        let is_first_ever = ux.last_active_day == 0;
        require!(ux.last_active_day < now_day, ErrorCode::AlreadyClaimedToday);

        const AM: u64 = 50_000_000; // 0.05
        if ctx.accounts.bootstrap_pool_token.amount >= AM {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.key(),
                    Transfer {
                        from: ctx.accounts.bootstrap_pool_token.to_account_info(),
                        to: ctx.accounts.user_token_account.to_account_info(),
                        authority: ctx.accounts.bootstrap_pool.to_account_info(),
                    },
                    &[&[b"bootstrap-pool", &[ctx.bumps.bootstrap_pool]]],
                ),
                AM,
            )?;
        } else {
            require!(
                ctx.accounts.reward_pool_token.amount >= AM,
                ErrorCode::InsufficientRewardPool
            );
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.key(),
                    Transfer {
                        from: ctx.accounts.reward_pool_token.to_account_info(),
                        to: ctx.accounts.user_token_account.to_account_info(),
                        authority: ctx.accounts.reward_pool.to_account_info(),
                    },
                    &[&[b"reward-pool", &[ctx.bumps.reward_pool]]],
                ),
                AM,
            )?;
        }
        if is_first_ever && cfg.minted_phase1 < 1_000 {
            cfg.minted_phase1 = cfg.minted_phase1.saturating_add(1);
        }
        ux.last_active_day = now_day;
        ux.bump = ctx.bumps.user_exercise;
        Ok(())
    }

    pub fn log_workout(
        ctx: Context<LogWorkout>,
        sets: u32,
        distance_walked: u64,
        distance_ran: u64,
    ) -> Result<()> {
        verify_seeker_genesis_ownership(
            &ctx.accounts.genesis_gate.seeker_genesis_mint,
            &ctx.accounts.user,
            &ctx.accounts.user_genesis_ata,
        )?;
        let s = &mut ctx.accounts.user_state;
        s.sets_completed = s.sets_completed.saturating_add(sets);
        s.total_distance_walked = s.total_distance_walked.saturating_add(distance_walked);
        s.total_distance_ran = s.total_distance_ran.saturating_add(distance_ran);
        s.last_workout = Clock::get()?.unix_timestamp;
        s.bump = ctx.bumps.user_state;
        Ok(())
    }

    pub fn update_daily_news(ctx: Context<UpdateDailyNews>, news_json: String) -> Result<()> {
        require!(news_json.len() <= 34_000, ErrorCode::NewsTooLarge);
        let n = &mut ctx.accounts.daily_news;
        n.data = news_json.into_bytes();
        n.last_updated = Clock::get()?.unix_timestamp;
        n.bump = ctx.bumps.daily_news;
        n.authority = ctx.accounts.user.key();
        Ok(())
    }

    pub fn reset_daily_news(ctx: Context<ResetDailyNews>) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.daily_news.authority,
            ctx.accounts.user.key(),
            ErrorCode::Unauthorized
        );
        ctx.accounts.daily_news.data = vec![];
        Ok(())
    }

    /// Returns 1000 minus the number of *first* successful claimers recorded in `minted_phase1` (capped at 1000).
    pub fn get_available_legend_slots(
        _ctx: Context<GetAvailableLegendSlots>,
        _exercise_id: String,
    ) -> Result<u32> {
        let m = _ctx.accounts.mint_config.minted_phase1;
        Ok(1_000u32.saturating_sub(m))
    }

    /// One-time: global **legend instrument** supply counter (cap `LEGEND_INSTRUMENT_CAP`). Same authority as `mint-config`.
    pub fn initialize_legend_supply(ctx: Context<InitializeLegendSupply>) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.mint_config.authority,
            ctx.accounts.user.key(),
            ErrorCode::Unauthorized
        );
        let ls = &mut ctx.accounts.legend_supply;
        ls.minted = 0;
        ls.bump = ctx.bumps.legend_supply;
        msg!("legend supply init");
        Ok(())
    }

    /// Admin: increment legend mint counter (for future `mint_sbt` / Bubblegum path). Fails past cap.
    pub fn record_legend_mint(ctx: Context<RecordLegendMint>, n: u32) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.mint_config.authority,
            ctx.accounts.user.key(),
            ErrorCode::Unauthorized
        );
        require!(n > 0, ErrorCode::InvalidLegendMintCount);
        let ls = &mut ctx.accounts.legend_supply;
        let next = ls.minted.saturating_add(n);
        require!(next <= LEGEND_INSTRUMENT_CAP, ErrorCode::LegendSupplyCapReached);
        ls.minted = next;
        Ok(())
    }

    /// Remaining legend instrument mints before global cap.
    pub fn get_legend_mint_remaining(
        _ctx: Context<GetLegendMintRemaining>,
    ) -> Result<u32> {
        let m = _ctx.accounts.legend_supply.minted;
        Ok(LEGEND_INSTRUMENT_CAP.saturating_sub(m))
    }

    /// One-time: stores the Seeker genesis **mint** pubkey checked by `log_workout` / `claim_daily_skr`.
    /// Use mainnet `GT22…` on mainnet-beta; use a devnet Token-2022 mint on devnet (separate from mainnet SKR).
    pub fn initialize_genesis_gate(
        ctx: Context<InitializeGenesisGate>,
        seeker_genesis_mint: Pubkey,
    ) -> Result<()> {
        require!(seeker_genesis_mint != Pubkey::default(), ErrorCode::Unauthorized);
        let g = &mut ctx.accounts.genesis_gate;
        g.authority = ctx.accounts.user.key();
        g.seeker_genesis_mint = seeker_genesis_mint;
        g.bump = ctx.bumps.genesis_gate;
        msg!("genesis gate ok");
        Ok(())
    }

    pub fn set_seeker_genesis_mint(
        ctx: Context<SetSeekerGenesisMint>,
        seeker_genesis_mint: Pubkey,
    ) -> Result<()> {
        require!(seeker_genesis_mint != Pubkey::default(), ErrorCode::Unauthorized);
        require_keys_eq!(
            ctx.accounts.genesis_gate.authority,
            ctx.accounts.user.key(),
            ErrorCode::Unauthorized
        );
        ctx.accounts.genesis_gate.seeker_genesis_mint = seeker_genesis_mint;
        Ok(())
    }

    // ==================== POOL SPLIT (post-threshold) — sub-ix for treasury roadmap ====================
    /// One-time: PDA `[b"stake-vault"]` metadata (matches app / admin ATA owner seed `stake-vault`).
    pub fn initialize_stake_vault(ctx: Context<InitializeStakeVault>) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.mint_config.authority,
            ctx.accounts.user.key(),
            ErrorCode::Unauthorized
        );
        let s = &mut ctx.accounts.stake_vault;
        s.authority = ctx.accounts.user.key();
        s.bump = ctx.bumps.stake_vault;
        msg!("stake vault pda ok");
        Ok(())
    }

    /// One-time: split policy PDA `[b"pool-split"]` — **excess** over `threshold_raw` goes 50% stake / 25% bootstrap / 25% treasury.
    pub fn initialize_pool_split_config(
        ctx: Context<InitializePoolSplitConfig>,
        threshold_raw: u64,
        treasury: Pubkey,
    ) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.mint_config.authority,
            ctx.accounts.user.key(),
            ErrorCode::Unauthorized
        );
        require!(treasury != Pubkey::default(), ErrorCode::Unauthorized);
        require!(threshold_raw > 0, ErrorCode::Unauthorized);
        let c = &mut ctx.accounts.pool_split;
        c.threshold_raw = threshold_raw;
        c.treasury = treasury;
        c.bump = ctx.bumps.pool_split;
        msg!("pool split config ok");
        Ok(())
    }

    pub fn update_pool_split_threshold(ctx: Context<UpdatePoolSplitConfig>, threshold_raw: u64) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.mint_config.authority,
            ctx.accounts.user.key(),
            ErrorCode::Unauthorized
        );
        require!(threshold_raw > 0, ErrorCode::Unauthorized);
        ctx.accounts.pool_split.threshold_raw = threshold_raw;
        Ok(())
    }

    pub fn update_pool_split_treasury(ctx: Context<UpdatePoolSplitConfig>, treasury: Pubkey) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.mint_config.authority,
            ctx.accounts.user.key(),
            ErrorCode::Unauthorized
        );
        require!(treasury != Pubkey::default(), ErrorCode::Unauthorized);
        ctx.accounts.pool_split.treasury = treasury;
        Ok(())
    }

    /// Move **only** `reward_pool_token.amount - threshold_raw` when positive: **50%** → stake vault ATA, **25%** → bootstrap pool ATA, **25%** → treasury ATA (owner must match `pool_split.treasury`).
    /// Does **not** stake into an external protocol; it is a one-shot rebalance between SPL accounts the authority controls.
    pub fn split_reward_pool_excess(ctx: Context<SplitRewardPoolExcess>) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.mint_config.authority,
            ctx.accounts.user.key(),
            ErrorCode::Unauthorized
        );
        require_keys_eq!(
            ctx.accounts.treasury_token.owner,
            ctx.accounts.pool_split.treasury,
            ErrorCode::TreasuryMismatch
        );
        require_keys_eq!(
            ctx.accounts.treasury_token.mint,
            ctx.accounts.skr_mint.key(),
            ErrorCode::InvalidTokenAccount
        );

        let bal = ctx.accounts.reward_pool_token.amount;
        let threshold = ctx.accounts.pool_split.threshold_raw;
        let excess = bal.checked_sub(threshold).ok_or(error!(ErrorCode::BelowSplitThreshold))?;
        require!(excess > 0, ErrorCode::BelowSplitThreshold);

        let e = excess as u128;
        let to_stake = u64::try_from(e * 50u128 / 100u128).map_err(|_| error!(ErrorCode::SplitMathOverflow))?;
        let to_bootstrap = u64::try_from(e * 25u128 / 100u128).map_err(|_| error!(ErrorCode::SplitMathOverflow))?;
        let to_treasury = excess
            .checked_sub(to_stake)
            .and_then(|x| x.checked_sub(to_bootstrap))
            .ok_or(error!(ErrorCode::SplitMathOverflow))?;

        let bump_reward = &[ctx.bumps.reward_pool];
        let seeds_reward: &[&[u8]] = &[b"reward-pool", bump_reward];
        let signer_reward = &[seeds_reward];

        if to_stake > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.key(),
                    Transfer {
                        from: ctx.accounts.reward_pool_token.to_account_info(),
                        to: ctx.accounts.stake_vault_token.to_account_info(),
                        authority: ctx.accounts.reward_pool.to_account_info(),
                    },
                    signer_reward,
                ),
                to_stake,
            )?;
        }
        if to_bootstrap > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.key(),
                    Transfer {
                        from: ctx.accounts.reward_pool_token.to_account_info(),
                        to: ctx.accounts.bootstrap_pool_token.to_account_info(),
                        authority: ctx.accounts.reward_pool.to_account_info(),
                    },
                    signer_reward,
                ),
                to_bootstrap,
            )?;
        }
        if to_treasury > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.key(),
                    Transfer {
                        from: ctx.accounts.reward_pool_token.to_account_info(),
                        to: ctx.accounts.treasury_token.to_account_info(),
                        authority: ctx.accounts.reward_pool.to_account_info(),
                    },
                    signer_reward,
                ),
                to_treasury,
            )?;
        }
        msg!(
            "pool split: excess={} stake={} boot={} treas={}",
            excess,
            to_stake,
            to_bootstrap,
            to_treasury
        );
        Ok(())
    }

    // ==================== SBT / LEGEND (merged from archive; genesis-gate + cohort caps) ====================
    /// Mint Legend (free while `mint_config.minted_phase1 < 1_000`) or paid SBT (1 SKR split: 50% stake vault, 25% reward pool, 25% dev).
    /// Increments **`legend_supply.minted`** (10k instrument cap). Requires **`initialize_stake_vault`** + stake-vault ATA for paid path CPIs.
    pub fn mint_sbt(
        ctx: Context<MintSbt>,
        _exercise_id: String,
        _video_id: String,
        label1: String,
        label2: String,
    ) -> Result<()> {
        verify_seeker_genesis_ownership(
            &ctx.accounts.genesis_gate.seeker_genesis_mint,
            &ctx.accounts.user,
            &ctx.accounts.user_genesis_ata,
        )?;
        require!(_exercise_id.len() <= 16, ErrorCode::InvalidExerciseId);
        require!(_video_id.len() <= 32, ErrorCode::InvalidExerciseId);
        require!(label1.len() <= 64, ErrorCode::InvalidExerciseId);
        require!(label2.len() <= 64, ErrorCode::InvalidExerciseId);

        require_keys_eq!(ctx.accounts.sbt.owner, Pubkey::default(), ErrorCode::SbtAlreadyMinted);

        let ls = &mut ctx.accounts.legend_supply;
        require!(
            ls.minted < LEGEND_INSTRUMENT_CAP,
            ErrorCode::LegendSupplyCapReached
        );

        let now = Clock::get()?.unix_timestamp;
        let now_day = claim_day(now);
        let cfg = &mut ctx.accounts.mint_config;
        let is_free_cohort = cfg.minted_phase1 < 1_000;

        if is_free_cohort {
            cfg.minted_phase1 = cfg.minted_phase1.saturating_add(1);
        } else {
            const AMOUNT: u64 = 1_000_000_000;
            require!(
                ctx.accounts.user_token_account.amount >= AMOUNT,
                ErrorCode::InsufficientUserSkr
            );
            let half = AMOUNT / 2;
            let quarter = AMOUNT / 4;
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.key(),
                    Transfer {
                        from: ctx.accounts.user_token_account.to_account_info(),
                        to: ctx.accounts.stake_vault_token.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                half,
            )?;
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.key(),
                    Transfer {
                        from: ctx.accounts.user_token_account.to_account_info(),
                        to: ctx.accounts.reward_pool_token.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                quarter,
            )?;
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.key(),
                    Transfer {
                        from: ctx.accounts.user_token_account.to_account_info(),
                        to: ctx.accounts.dev_wallet_token.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                quarter,
            )?;
        }

        ls.minted = ls.minted.saturating_add(1);

        let sbt = &mut ctx.accounts.sbt;
        sbt.owner = ctx.accounts.user.key();
        sbt.uri = String::new();
        sbt.label1 = label1;
        sbt.label2 = label2;
        sbt.version = 1;
        sbt.is_early = is_free_cohort;
        sbt.bump = ctx.bumps.sbt;
        sbt.last_updated = now;
        sbt.data_version = 1;
        sbt.total_sets_completed = 0;
        sbt.total_distance_walked = 0;
        sbt.total_distance_ran = 0;
        sbt.encrypted_fitness_data = vec![];

        let ux = &mut ctx.accounts.user_exercise;
        ux.last_active_day = now_day;
        ux.bump = ctx.bumps.user_exercise;
        if is_free_cohort {
            ux.is_legend = true;
            ux.extension_paid_until = now
                .checked_add(30_i64.saturating_mul(86400))
                .ok_or(error!(ErrorCode::TimestampOverflow))?;
        }
        Ok(())
    }

    /// Call in the **same transaction** immediately after `mint_sbt` (keeps `MintSbt` stack under BPF limits).
    /// One-time `init` of `[b"legend-entitlement", user, exercise_id]` from current `sbt` + `user_exercise`.
    pub fn record_legend_entitlement(
        ctx: Context<RecordLegendEntitlement>,
        exercise_id: String,
    ) -> Result<()> {
        require!(exercise_id.len() <= 16, ErrorCode::InvalidExerciseId);
        require_keys_eq!(ctx.accounts.sbt.owner, ctx.accounts.user.key(), ErrorCode::Unauthorized);
        let exb = exercise_id.as_bytes();
        let mut buf = [0u8; 16];
        buf[..exb.len()].copy_from_slice(exb);
        let ux = &ctx.accounts.user_exercise;
        let ent = &mut ctx.accounts.legend_entitlement;
        ent.exercise_id = buf;
        ent.exercise_len = exb.len() as u8;
        ent.visual_owner = ctx.accounts.user.key();
        ent.status_holder = ctx.accounts.user.key();
        ent.extension_paid_until = ux.extension_paid_until;
        ent.bump = ctx.bumps.legend_entitlement;
        Ok(())
    }

    /// Free 30-day extension for the **current `status_holder`** (genesis-gated). Updates `LegendEntitlement` only (no SKR).
    pub fn extend_legend(ctx: Context<ExtendLegend>, exercise_id: String) -> Result<()> {
        verify_seeker_genesis_ownership(
            &ctx.accounts.genesis_gate.seeker_genesis_mint,
            &ctx.accounts.user,
            &ctx.accounts.user_genesis_ata,
        )?;
        require_exercise_id_matches_entitlement(&ctx.accounts.legend_entitlement, &exercise_id)?;
        let now = Clock::get()?.unix_timestamp;
        let ent = &mut ctx.accounts.legend_entitlement;
        ent.extension_paid_until = now
            .checked_add(30_i64.saturating_mul(86400))
            .ok_or(error!(ErrorCode::TimestampOverflow))?;
        Ok(())
    }

    /// **`visual_owner`** (image / SBT owner) reassigns **Legend status** to `new_status_holder`. Clears `is_legend` on the visual owner's `user_exercise`.
    /// **Escrow:** trustless sale can wrap this ix (CPI or same-tx bundle) without changing entitlement PDAs; optional future ix may add an escrow signer.
    pub fn transfer_legend_status(
        ctx: Context<TransferLegendStatus>,
        exercise_id: String,
        new_status_holder: Pubkey,
    ) -> Result<()> {
        require!(new_status_holder != Pubkey::default(), ErrorCode::Unauthorized);
        require_exercise_id_matches_entitlement(&ctx.accounts.legend_entitlement, &exercise_id)?;
        let ent = &mut ctx.accounts.legend_entitlement;
        ent.status_holder = new_status_holder;
        let ux = &mut ctx.accounts.visual_owner_user_exercise;
        ux.is_legend = false;
        ux.extension_paid_until = 0;
        Ok(())
    }

    /// Buyer / new holder syncs Legend status into their **`user_exercise`** (same exercise id as entitlement).
    /// **Escrow:** same-tx or CPI after payment release; no escrow state stored here by design.
    pub fn accept_legend_status(ctx: Context<AcceptLegendStatus>, exercise_id: String) -> Result<()> {
        require_exercise_id_matches_entitlement(&ctx.accounts.legend_entitlement, &exercise_id)?;
        require_keys_eq!(
            ctx.accounts.legend_entitlement.status_holder,
            ctx.accounts.user.key(),
            ErrorCode::Unauthorized
        );
        let now = Clock::get()?.unix_timestamp;
        let now_day = claim_day(now);
        let ent_ext = ctx.accounts.legend_entitlement.extension_paid_until;
        let ux = &mut ctx.accounts.user_exercise;
        ux.is_legend = true;
        ux.extension_paid_until = ent_ext;
        ux.last_active_day = now_day;
        ux.bump = ctx.bumps.user_exercise;
        Ok(())
    }

    /// List **Legend status** for sale at a fixed SKR price. Requires `status_holder == visual_owner` (cancel listing before using P2P `transfer_legend_status`).
    pub fn list_legend_sale(
        ctx: Context<ListLegendSale>,
        exercise_id: String,
        price_lamports: u64,
    ) -> Result<()> {
        require!(price_lamports > 0, ErrorCode::LegendSaleInvalidPrice);
        require_exercise_id_matches_entitlement(&ctx.accounts.legend_entitlement, &exercise_id)?;
        let ent = &ctx.accounts.legend_entitlement;
        require_keys_eq!(
            ent.status_holder,
            ent.visual_owner,
            ErrorCode::Unauthorized
        );
        require_keys_eq!(ent.visual_owner, ctx.accounts.visual_owner.key(), ErrorCode::Unauthorized);

        let sale = &mut ctx.accounts.legend_sale;
        require!(
            sale.state != LEGEND_SALE_LISTED,
            ErrorCode::LegendSaleAlreadyListed
        );
        sale.visual_owner = ent.visual_owner;
        sale.exercise_id = ent.exercise_id;
        sale.exercise_len = ent.exercise_len;
        sale.price_lamports = price_lamports;
        sale.state = LEGEND_SALE_LISTED;
        sale.bump = ctx.bumps.legend_sale;
        Ok(())
    }

    pub fn cancel_legend_sale(ctx: Context<CancelLegendSale>, exercise_id: String) -> Result<()> {
        require_exercise_id_matches_sale(&ctx.accounts.legend_sale, &exercise_id)?;
        require_keys_eq!(
            ctx.accounts.legend_sale.visual_owner,
            ctx.accounts.visual_owner.key(),
            ErrorCode::Unauthorized
        );
        let sale = &mut ctx.accounts.legend_sale;
        require!(
            sale.state == LEGEND_SALE_LISTED,
            ErrorCode::LegendSaleNotListed
        );
        sale.state = LEGEND_SALE_CANCELLED;
        Ok(())
    }

    /// Buyer pays **`price_lamports` SKR** to `visual_owner`, receives **`status_holder`**, sale PDA closes (rent → buyer).
    pub fn buy_legend_sale(ctx: Context<BuyLegendSale>, exercise_id: String) -> Result<()> {
        require_exercise_id_matches_entitlement(&ctx.accounts.legend_entitlement, &exercise_id)?;
        require_exercise_id_matches_sale(&ctx.accounts.legend_sale, &exercise_id)?;
        require_keys_eq!(
            ctx.accounts.legend_sale.visual_owner,
            ctx.accounts.legend_entitlement.visual_owner,
            ErrorCode::Unauthorized
        );
        require!(
            ctx.accounts.legend_sale.state == LEGEND_SALE_LISTED,
            ErrorCode::LegendSaleNotListed
        );
        let price = ctx.accounts.legend_sale.price_lamports;
        require!(
            ctx.accounts.buyer_token.amount >= price,
            ErrorCode::InsufficientUserSkr
        );
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                Transfer {
                    from: ctx.accounts.buyer_token.to_account_info(),
                    to: ctx.accounts.seller_token.to_account_info(),
                    authority: ctx.accounts.buyer.to_account_info(),
                },
            ),
            price,
        )?;
        let ent = &mut ctx.accounts.legend_entitlement;
        ent.status_holder = ctx.accounts.buyer.key();
        let ux = &mut ctx.accounts.visual_owner_user_exercise;
        ux.is_legend = false;
        ux.extension_paid_until = 0;
        Ok(())
    }

    // ==================== TREASURY POLICY (multisig + timelock) ====================
    /// One-time PDA `[b"treasury-policy"]`: **intent** for splitting treasury SKR staking yield — **distribute** (`distribute_bps`) vs **compound** (`BPS_DENOM - distribute_bps`).
    /// **Canonical model:** pair with **multisig + timelock** (e.g. Squads/Safe) for real stake/transfer txs; here `authority` = that multisig vault, `min_change_interval_sec` = timelock between **policy** edits.
    /// Does not move tokens or CPI to Solana Mobile staking; ops execute SPL flows through the multisig.
    pub fn initialize_treasury_policy(
        ctx: Context<InitializeTreasuryPolicy>,
        authority: Pubkey,
        distribute_bps: u16,
        min_change_interval_sec: u64,
    ) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.mint_config.authority,
            ctx.accounts.user.key(),
            ErrorCode::Unauthorized
        );
        require!(authority != Pubkey::default(), ErrorCode::Unauthorized);
        require!(distribute_bps <= BPS_DENOM, ErrorCode::InvalidTreasuryPolicy);
        let p = &mut ctx.accounts.treasury_policy;
        p.authority = authority;
        p.distribute_bps = distribute_bps;
        p.min_change_interval_sec = min_change_interval_sec;
        p.last_change_ts = 0;
        p.bump = ctx.bumps.treasury_policy;
        msg!("treasury policy ok");
        Ok(())
    }

    pub fn update_treasury_policy(ctx: Context<UpdateTreasuryPolicy>, distribute_bps: u16) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.user.key(),
            ctx.accounts.treasury_policy.authority,
            ErrorCode::Unauthorized
        );
        require!(distribute_bps <= BPS_DENOM, ErrorCode::InvalidTreasuryPolicy);
        assert_treasury_policy_timelock(
            ctx.accounts.treasury_policy.last_change_ts,
            ctx.accounts.treasury_policy.min_change_interval_sec,
        )?;
        let p = &mut ctx.accounts.treasury_policy;
        p.distribute_bps = distribute_bps;
        p.last_change_ts = Clock::get()?.unix_timestamp;
        Ok(())
    }

    pub fn update_treasury_policy_timelock(
        ctx: Context<UpdateTreasuryPolicy>,
        min_change_interval_sec: u64,
    ) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.user.key(),
            ctx.accounts.treasury_policy.authority,
            ErrorCode::Unauthorized
        );
        assert_treasury_policy_timelock(
            ctx.accounts.treasury_policy.last_change_ts,
            ctx.accounts.treasury_policy.min_change_interval_sec,
        )?;
        let p = &mut ctx.accounts.treasury_policy;
        p.min_change_interval_sec = min_change_interval_sec;
        p.last_change_ts = Clock::get()?.unix_timestamp;
        Ok(())
    }

    pub fn set_treasury_policy_authority(
        ctx: Context<UpdateTreasuryPolicy>,
        new_authority: Pubkey,
    ) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.user.key(),
            ctx.accounts.treasury_policy.authority,
            ErrorCode::Unauthorized
        );
        require!(new_authority != Pubkey::default(), ErrorCode::Unauthorized);
        assert_treasury_policy_timelock(
            ctx.accounts.treasury_policy.last_change_ts,
            ctx.accounts.treasury_policy.min_change_interval_sec,
        )?;
        let p = &mut ctx.accounts.treasury_policy;
        p.authority = new_authority;
        p.last_change_ts = Clock::get()?.unix_timestamp;
        Ok(())
    }
}

// ==================== ACCOUNTS (trimmed) ====================
#[account]
pub struct MintConfig {
    pub authority: Pubkey,
    pub phase: u8,
    /// First unique successful claimers recorded (capped 1000). Same field as pre-minimal; pool still depletes by amount on-chain.
    pub minted_phase1: u32,
    pub minted_phase2: u32,
    pub rewards_enabled: bool,
    pub bump: u8,
}

/// Global gate: which Token-2022 (or spl-token) mint counts as “Seeker genesis” for this deployment.
#[account]
pub struct GenesisGateConfig {
    pub authority: Pubkey,
    pub seeker_genesis_mint: Pubkey,
    pub bump: u8,
}

/// Global legend-instrument mint counter (not claim cohort / not frame cNFTs).
#[account]
pub struct LegendSupply {
    pub minted: u32,
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

/// PDA + its SKR ATA — liquidity for daily claims **after** the bootstrap pool is drawn down. Idle SKR (no on-chain yield).
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

/// PDA `[b"stake-vault"]` — SKR ATA that receives the **50%** leg of `split_reward_pool_excess`.
/// Naming reflects **policy** (long-term staking allocation), not an integrated stake program: no CPI to a stake pool here.
#[account]
pub struct StakeVault {
    pub authority: Pubkey,
    pub bump: u8,
}

/// PDA `[b"pool-split"]` — threshold + treasury recipient for `split_reward_pool_excess`.
#[account]
pub struct PoolSplitConfig {
    /// Raw SKR (lamports, 9 decimals) **floor** left in reward pool; only **excess** is splittable.
    pub threshold_raw: u64,
    /// Owner of the treasury **SKR** token account (25% leg).
    pub treasury: Pubkey,
    pub bump: u8,
}

/// PDA `[b"treasury-policy"]` — **multisig + timelock on policy edits** for treasury SKR yield intent (distribute vs compound).
#[account]
pub struct TreasuryPolicyConfig {
    /// Basis points of staking yield intended for **distribution** (pools, grants, etc.). **Compound** intent = `BPS_DENOM - distribute_bps`.
    pub distribute_bps: u16,
    /// Minimum seconds between policy **updates** (after first update). `0` disables timelock.
    pub min_change_interval_sec: u64,
    /// Unix timestamp of last successful `update_*` / `set_treasury_policy_authority`. `0` = never updated yet.
    pub last_change_ts: i64,
    /// Multisig vault (or other) pubkey that must sign policy changes.
    pub authority: Pubkey,
    pub bump: u8,
}

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

/// PDA `[b"legend-entitlement", visual_owner, exercise_id]` — transferable **Legend status** (who may extend / claim perks).
/// **`visual_owner`** keeps the SBT / image PDA; **`status_holder`** may differ after `transfer_legend_status`.
///
/// **Escrow later:** keep these seeds stable; a separate escrow program (or a new ix here) should gate *who signs*
/// `transfer_legend_status` / `accept_legend_status` — do not move `visual_owner` out of the PDA seed without a migration plan.
#[account]
pub struct LegendEntitlement {
    /// Wallet that owns the SBT / image (does not move on status transfer).
    pub visual_owner: Pubkey,
    /// Wallet that currently holds Legend status (extensions, app reads).
    pub status_holder: Pubkey,
    pub exercise_id: [u8; 16],
    pub exercise_len: u8,
    pub extension_paid_until: i64,
    pub bump: u8,
}

fn require_exercise_id_matches_entitlement(ent: &LegendEntitlement, id: &str) -> Result<()> {
    let b = id.as_bytes();
    require!(b.len() <= 16, ErrorCode::InvalidExerciseId);
    require!(
        b.len() == ent.exercise_len as usize,
        ErrorCode::LegendExerciseMismatch
    );
    require!(
        ent.exercise_id[..b.len()] == *b,
        ErrorCode::LegendExerciseMismatch
    );
    Ok(())
}

fn require_exercise_id_matches_sale(sale: &LegendSale, id: &str) -> Result<()> {
    let b = id.as_bytes();
    require!(b.len() <= 16, ErrorCode::InvalidExerciseId);
    require!(
        b.len() == sale.exercise_len as usize,
        ErrorCode::LegendExerciseMismatch
    );
    require!(
        sale.exercise_id[..b.len()] == *b,
        ErrorCode::LegendExerciseMismatch
    );
    Ok(())
}

/// PDA `[b"legend-sale", visual_owner, exercise_id]` — trustless SKR listing for **Legend status** (`buy_legend_sale`).
#[account]
pub struct LegendSale {
    pub visual_owner: Pubkey,
    pub exercise_id: [u8; 16],
    pub exercise_len: u8,
    pub price_lamports: u64,
    pub state: u8,
    pub bump: u8,
}

/// User-owned SBT PDA (`user` + `user-sbt` + `exercise_id` seeds) — Legend / paid mint metadata + optional fitness payload.
#[account]
pub struct SbtAccount {
    pub owner: Pubkey,
    pub uri: String,
    pub label1: String,
    pub label2: String,
    pub version: u8,
    /// True when minted under the **first 1_000 cohort** (`mint_config.minted_phase1` path), same notion as claims.
    pub is_early: bool,
    pub bump: u8,
    pub total_sets_completed: u32,
    pub total_distance_walked: u64,
    pub total_distance_ran: u64,
    pub encrypted_fitness_data: Vec<u8>,
    pub last_updated: i64,
    pub data_version: u8,
}

// === contexts ===
#[derive(Accounts)]
pub struct InitializeGenesisGate<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + 32 + 32 + 1,
        seeds = [b"genesis-gate"],
        bump
    )]
    pub genesis_gate: Account<'info, GenesisGateConfig>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetSeekerGenesisMint<'info> {
    #[account(mut, seeds = [b"genesis-gate"], bump = genesis_gate.bump)]
    pub genesis_gate: Account<'info, GenesisGateConfig>,
    pub user: Signer<'info>,
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
pub struct ToggleRewards<'info> {
    #[account(mut, seeds = [b"mint-config"], bump)]
    pub mint_config: Account<'info, MintConfig>,
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct AdminMintSKR<'info> {
    #[account(mut, seeds = [b"mint-config"], bump)]
    pub mint_config: Account<'info, MintConfig>,
    #[account(mut, address = pubkey!("DCrfzg5T8hijkX8EM6oN9sh4Ucm1AMqqNZQZBGTbmofQ"))]
    pub skr_mint: Account<'info, Mint>,
    #[account(mut)]
    pub destination: Account<'info, TokenAccount>,
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct GetAvailableLegendSlots<'info> {
    #[account(seeds = [b"mint-config"], bump)]
    pub mint_config: Account<'info, MintConfig>,
}

#[derive(Accounts)]
pub struct InitializeLegendSupply<'info> {
    #[account(seeds = [b"mint-config"], bump)]
    pub mint_config: Account<'info, MintConfig>,
    #[account(
        init,
        payer = user,
        space = 8 + 4 + 1,
        seeds = [b"legend-supply"],
        bump
    )]
    pub legend_supply: Account<'info, LegendSupply>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RecordLegendMint<'info> {
    #[account(seeds = [b"mint-config"], bump)]
    pub mint_config: Account<'info, MintConfig>,
    #[account(mut, seeds = [b"legend-supply"], bump = legend_supply.bump)]
    pub legend_supply: Account<'info, LegendSupply>,
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct GetLegendMintRemaining<'info> {
    #[account(seeds = [b"legend-supply"], bump = legend_supply.bump)]
    pub legend_supply: Account<'info, LegendSupply>,
}

#[derive(Accounts)]
pub struct LogWorkout<'info> {
    #[account(
        init_if_needed,
        payer = user,
        // 8 disc + UserState: u32 + u64×3 + i64×2 + u32 + u8 = 8 + 41 = 49 (was missing one i64 for last_claim)
        space = 8 + 4 + 8 + 8 + 8 + 8 + 4 + 1,
        seeds = [b"user-state", user.key().as_ref()],
        bump
    )]
    pub user_state: Account<'info, UserState>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(seeds = [b"genesis-gate"], bump = genesis_gate.bump)]
    pub genesis_gate: Account<'info, GenesisGateConfig>,
    #[account(
        constraint = genesis_mint.key() == genesis_gate.seeker_genesis_mint @ ErrorCode::MissingGenesisNft
    )]
    pub genesis_mint: InterfaceAccount<'info, GenesisMintTy>,
    #[account(constraint = user_genesis_ata.mint == genesis_mint.key(), constraint = user_genesis_ata.owner == user.key())]
    pub user_genesis_ata: InterfaceAccount<'info, GenesisTokenAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimDailySKR<'info> {
    #[account(mut, seeds = [b"mint-config"], bump)]
    pub mint_config: Account<'info, MintConfig>,
    #[account(seeds = [b"genesis-gate"], bump = genesis_gate.bump)]
    pub genesis_gate: Account<'info, GenesisGateConfig>,
    /// Must have `sets_completed >= 1` from a prior `log_workout` (enforced in handler).
    #[account(seeds = [b"user-state", user.key().as_ref()], bump)]
    pub user_state: Account<'info, UserState>,
    #[account(mut, seeds = [b"bootstrap-pool"], bump)]
    pub bootstrap_pool: Account<'info, BootstrapPool>,
    #[account(mut, associated_token::mint = skr_mint, associated_token::authority = bootstrap_pool)]
    pub bootstrap_pool_token: Box<Account<'info, TokenAccount>>,
    #[account(mut, seeds = [b"reward-pool"], bump)]
    pub reward_pool: Account<'info, RewardPool>,
    #[account(mut, associated_token::mint = skr_mint, associated_token::authority = reward_pool)]
    pub reward_pool_token: Box<Account<'info, TokenAccount>>,
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 4 + 8 + 8 + 8 + 4 + 8 + 1 + 1,
        seeds = [b"user-exercise", user.key().as_ref(), b"sprint-interval".as_ref()],
        bump
    )]
    pub user_exercise: Box<Account<'info, UserExerciseState>>,
    #[account(mut, associated_token::mint = skr_mint, associated_token::authority = user)]
    pub user_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub skr_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    #[account(
        constraint = genesis_mint.key() == genesis_gate.seeker_genesis_mint @ ErrorCode::MissingGenesisNft
    )]
    pub genesis_mint: InterfaceAccount<'info, GenesisMintTy>,
    #[account(constraint = user_genesis_ata.mint == genesis_mint.key(), constraint = user_genesis_ata.owner == user.key())]
    pub user_genesis_ata: Box<InterfaceAccount<'info, GenesisTokenAccount>>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateDailyNews<'info> {
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 32 + 4 + 34_000 + 8 + 1,
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
    #[account(mut, seeds = [b"daily-news-seeker-final"], bump)]
    pub daily_news: Account<'info, DailyNews>,
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct InitializeStakeVault<'info> {
    #[account(seeds = [b"mint-config"], bump)]
    pub mint_config: Account<'info, MintConfig>,
    #[account(
        init,
        payer = user,
        space = 8 + 32 + 1,
        seeds = [b"stake-vault"],
        bump
    )]
    pub stake_vault: Account<'info, StakeVault>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializePoolSplitConfig<'info> {
    #[account(seeds = [b"mint-config"], bump)]
    pub mint_config: Account<'info, MintConfig>,
    #[account(
        init,
        payer = user,
        space = 8 + 8 + 32 + 1,
        seeds = [b"pool-split"],
        bump
    )]
    pub pool_split: Account<'info, PoolSplitConfig>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdatePoolSplitConfig<'info> {
    #[account(seeds = [b"mint-config"], bump)]
    pub mint_config: Account<'info, MintConfig>,
    #[account(mut, seeds = [b"pool-split"], bump = pool_split.bump)]
    pub pool_split: Account<'info, PoolSplitConfig>,
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct SplitRewardPoolExcess<'info> {
    #[account(seeds = [b"mint-config"], bump)]
    pub mint_config: Box<Account<'info, MintConfig>>,
    #[account(seeds = [b"pool-split"], bump = pool_split.bump)]
    pub pool_split: Box<Account<'info, PoolSplitConfig>>,
    #[account(mut, seeds = [b"reward-pool"], bump)]
    pub reward_pool: Box<Account<'info, RewardPool>>,
    #[account(mut, associated_token::mint = skr_mint, associated_token::authority = reward_pool)]
    pub reward_pool_token: Box<Account<'info, TokenAccount>>,
    #[account(mut, seeds = [b"bootstrap-pool"], bump)]
    pub bootstrap_pool: Box<Account<'info, BootstrapPool>>,
    #[account(mut, associated_token::mint = skr_mint, associated_token::authority = bootstrap_pool)]
    pub bootstrap_pool_token: Box<Account<'info, TokenAccount>>,
    #[account(seeds = [b"stake-vault"], bump = stake_vault.bump)]
    pub stake_vault: Box<Account<'info, StakeVault>>,
    #[account(mut, associated_token::mint = skr_mint, associated_token::authority = stake_vault)]
    pub stake_vault_token: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub treasury_token: Box<Account<'info, TokenAccount>>,
    pub user: Signer<'info>,
    #[account(mut, address = pubkey!("DCrfzg5T8hijkX8EM6oN9sh4Ucm1AMqqNZQZBGTbmofQ"))]
    pub skr_mint: Box<Account<'info, Mint>>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(_exercise_id: String, _video_id: String)]
pub struct MintSbt<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [b"mint-config"], bump)]
    pub mint_config: Box<Account<'info, MintConfig>>,
    #[account(mut, seeds = [b"legend-supply"], bump = legend_supply.bump)]
    pub legend_supply: Box<Account<'info, LegendSupply>>,
    #[account(seeds = [b"genesis-gate"], bump = genesis_gate.bump)]
    pub genesis_gate: Box<Account<'info, GenesisGateConfig>>,
    #[account(
        constraint = genesis_mint.key() == genesis_gate.seeker_genesis_mint @ ErrorCode::MissingGenesisNft
    )]
    pub genesis_mint: Box<InterfaceAccount<'info, GenesisMintTy>>,
    #[account(constraint = user_genesis_ata.mint == genesis_mint.key(), constraint = user_genesis_ata.owner == user.key())]
    pub user_genesis_ata: Box<InterfaceAccount<'info, GenesisTokenAccount>>,
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 32 + 4 + 200 + 4 + 64 + 4 + 64 + 1 + 1 + 1 + 4 + 8 + 8 + 8 + 4 + 8 + 1,
        seeds = [user.key().as_ref(), b"user-sbt", _exercise_id.as_bytes()],
        bump
    )]
    pub sbt: Box<Account<'info, SbtAccount>>,
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 4 + 8 + 8 + 8 + 4 + 8 + 1 + 1,
        seeds = [b"user-exercise", user.key().as_ref(), _exercise_id.as_bytes()],
        bump
    )]
    pub user_exercise: Box<Account<'info, UserExerciseState>>,
    #[account(mut, associated_token::mint = skr_mint, associated_token::authority = user)]
    pub user_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut, associated_token::mint = skr_mint, associated_token::authority = reward_pool)]
    pub reward_pool_token: Box<Account<'info, TokenAccount>>,
    #[account(mut, seeds = [b"reward-pool"], bump)]
    pub reward_pool: Box<Account<'info, RewardPool>>,
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = skr_mint,
        associated_token::authority = stake_vault
    )]
    pub stake_vault_token: Box<Account<'info, TokenAccount>>,
    #[account(seeds = [b"stake-vault"], bump = stake_vault.bump)]
    pub stake_vault: Box<Account<'info, StakeVault>>,
    #[account(mut, associated_token::mint = skr_mint, associated_token::authority = dev_fee_authority)]
    pub dev_wallet_token: Box<Account<'info, TokenAccount>>,
    /// CHECK: Dev fee recipient pubkey; only used as ATA authority for `dev_wallet_token`.
    #[account(address = pubkey!("B9Qo6q398kvryKQuCUMjRxQHMbVTGTc3wwSbrRoKaTrc"))]
    pub dev_fee_authority: UncheckedAccount<'info>,
    #[account(mut, address = pubkey!("DCrfzg5T8hijkX8EM6oN9sh4Ucm1AMqqNZQZBGTbmofQ"))]
    pub skr_mint: Box<Account<'info, Mint>>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct RecordLegendEntitlement<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        seeds = [user.key().as_ref(), b"user-sbt", exercise_id.as_bytes()],
        bump = sbt.bump
    )]
    pub sbt: Account<'info, SbtAccount>,
    #[account(
        seeds = [b"user-exercise", user.key().as_ref(), exercise_id.as_bytes()],
        bump = user_exercise.bump
    )]
    pub user_exercise: Account<'info, UserExerciseState>,
    #[account(
        init,
        payer = user,
        space = 8 + 32 + 32 + 16 + 1 + 8 + 1,
        seeds = [b"legend-entitlement", user.key().as_ref(), exercise_id.as_bytes()],
        bump
    )]
    pub legend_entitlement: Account<'info, LegendEntitlement>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct ExtendLegend<'info> {
    /// CHECK: PDA seed only; must equal `legend_entitlement.visual_owner`.
    pub visual_owner: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [b"legend-entitlement", visual_owner.key().as_ref(), exercise_id.as_bytes()],
        bump = legend_entitlement.bump,
        constraint = legend_entitlement.visual_owner == visual_owner.key() @ ErrorCode::Unauthorized,
        constraint = legend_entitlement.status_holder == user.key() @ ErrorCode::Unauthorized,
    )]
    pub legend_entitlement: Box<Account<'info, LegendEntitlement>>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(seeds = [b"genesis-gate"], bump = genesis_gate.bump)]
    pub genesis_gate: Box<Account<'info, GenesisGateConfig>>,
    #[account(
        constraint = genesis_mint.key() == genesis_gate.seeker_genesis_mint @ ErrorCode::MissingGenesisNft
    )]
    pub genesis_mint: Box<InterfaceAccount<'info, GenesisMintTy>>,
    #[account(constraint = user_genesis_ata.mint == genesis_mint.key(), constraint = user_genesis_ata.owner == user.key())]
    pub user_genesis_ata: Box<InterfaceAccount<'info, GenesisTokenAccount>>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct TransferLegendStatus<'info> {
    #[account(mut)]
    pub visual_owner: Signer<'info>,
    #[account(
        mut,
        seeds = [b"legend-entitlement", visual_owner.key().as_ref(), exercise_id.as_bytes()],
        bump = legend_entitlement.bump,
        constraint = legend_entitlement.visual_owner == visual_owner.key() @ ErrorCode::Unauthorized,
    )]
    pub legend_entitlement: Box<Account<'info, LegendEntitlement>>,
    #[account(
        mut,
        seeds = [b"user-exercise", visual_owner.key().as_ref(), exercise_id.as_bytes()],
        bump = visual_owner_user_exercise.bump,
    )]
    pub visual_owner_user_exercise: Box<Account<'info, UserExerciseState>>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct AcceptLegendStatus<'info> {
    /// CHECK: PDA seed only; must equal `legend_entitlement.visual_owner`.
    pub visual_owner: UncheckedAccount<'info>,
    #[account(
        seeds = [b"legend-entitlement", visual_owner.key().as_ref(), exercise_id.as_bytes()],
        bump = legend_entitlement.bump,
        constraint = legend_entitlement.visual_owner == visual_owner.key() @ ErrorCode::Unauthorized,
        constraint = legend_entitlement.status_holder == user.key() @ ErrorCode::Unauthorized,
    )]
    pub legend_entitlement: Box<Account<'info, LegendEntitlement>>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 4 + 8 + 8 + 8 + 4 + 8 + 1 + 1,
        seeds = [b"user-exercise", user.key().as_ref(), exercise_id.as_bytes()],
        bump
    )]
    pub user_exercise: Box<Account<'info, UserExerciseState>>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct ListLegendSale<'info> {
    #[account(mut)]
    pub visual_owner: Signer<'info>,
    #[account(
        seeds = [b"legend-entitlement", visual_owner.key().as_ref(), exercise_id.as_bytes()],
        bump = legend_entitlement.bump,
        constraint = legend_entitlement.visual_owner == visual_owner.key() @ ErrorCode::Unauthorized,
    )]
    pub legend_entitlement: Box<Account<'info, LegendEntitlement>>,
    #[account(
        init_if_needed,
        payer = visual_owner,
        space = 8 + 32 + 16 + 1 + 8 + 1 + 1,
        seeds = [b"legend-sale", visual_owner.key().as_ref(), exercise_id.as_bytes()],
        bump
    )]
    pub legend_sale: Box<Account<'info, LegendSale>>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct CancelLegendSale<'info> {
    #[account(mut)]
    pub visual_owner: Signer<'info>,
    #[account(
        mut,
        seeds = [b"legend-sale", visual_owner.key().as_ref(), exercise_id.as_bytes()],
        bump = legend_sale.bump,
    )]
    pub legend_sale: Box<Account<'info, LegendSale>>,
}

#[derive(Accounts)]
#[instruction(exercise_id: String)]
pub struct BuyLegendSale<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,
    /// CHECK: must match `legend_entitlement.visual_owner` (SKR recipient).
    pub visual_owner: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [b"legend-entitlement", visual_owner.key().as_ref(), exercise_id.as_bytes()],
        bump = legend_entitlement.bump,
        constraint = legend_entitlement.visual_owner == visual_owner.key() @ ErrorCode::Unauthorized,
    )]
    pub legend_entitlement: Box<Account<'info, LegendEntitlement>>,
    #[account(
        mut,
        seeds = [b"legend-sale", visual_owner.key().as_ref(), exercise_id.as_bytes()],
        bump = legend_sale.bump,
        close = buyer
    )]
    pub legend_sale: Box<Account<'info, LegendSale>>,
    #[account(mut, associated_token::mint = skr_mint, associated_token::authority = buyer)]
    pub buyer_token: Box<Account<'info, TokenAccount>>,
    #[account(mut, associated_token::mint = skr_mint, associated_token::authority = visual_owner)]
    pub seller_token: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        seeds = [b"user-exercise", visual_owner.key().as_ref(), exercise_id.as_bytes()],
        bump = visual_owner_user_exercise.bump,
    )]
    pub visual_owner_user_exercise: Box<Account<'info, UserExerciseState>>,
    #[account(mut, address = pubkey!("DCrfzg5T8hijkX8EM6oN9sh4Ucm1AMqqNZQZBGTbmofQ"))]
    pub skr_mint: Box<Account<'info, Mint>>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct InitializeTreasuryPolicy<'info> {
    #[account(seeds = [b"mint-config"], bump)]
    pub mint_config: Account<'info, MintConfig>,
    #[account(
        init,
        payer = user,
        space = 8 + 2 + 8 + 8 + 32 + 1,
        seeds = [b"treasury-policy"],
        bump
    )]
    pub treasury_policy: Account<'info, TreasuryPolicyConfig>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateTreasuryPolicy<'info> {
    #[account(mut, seeds = [b"treasury-policy"], bump = treasury_policy.bump)]
    pub treasury_policy: Account<'info, TreasuryPolicyConfig>,
    pub user: Signer<'info>,
}
