//! 3/5 **upgrade** approvals + **execute_upgrade**; 3/5 (bits) to adjust **execute timelock** or **pause**; **5/5 + long timelock** for **irreversible** `close` (program) via CPI.
//!
//! ## Time model
//! - **Execute (upgrade) delay:** `timelock_duration` (seconds) is set with [`set_timelock`] while **≥3 approval bits** are set.
//!   After 3/5, `queued_at` is set. **Execute** is allowed when `now >= queued_at + timelock_duration` (if `timelock_duration > 0`).
//!   Set timelock **before** the third `approve` if you want a minimum delay; otherwise 3/5 can execute immediately.
//! - **Pause:** a **boolean**; there is **no** on-chain “pause for N seconds” timer. `pause=true` blocks **execute** until
//!   someone in **UPGRADE_SIGNERS** calls with `false` and **≥3 approval bits** (same as today).
//! - **“Cancel” queue:** not a named instruction. **[`set_timelock`]** or [`pause_upgrades`] clears `approvals` and `queued_at` (restarts).
//! - **Irreversible close (5/5 + timelock):** separate PDA state ([`IrreversibleClose`]). `propose_close_program` → each signer
//!   [`approve_irreversible`] bit → on 5/5, `irrev_queued_at` is set; **execute** after `irrev_timelock` (min 7d default).

#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_lang::solana_program::bpf_loader_upgradeable;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_lang::system_program;
use solana_loader_v3_interface::instruction::close_any;

declare_id!("73yx8W1HB7kooLXSgoqJtNoBNbRUKaotFNygd7b9dDRQ");

/// Minimum delay after 5/5 is reached before `execute_close_program` (seconds). Product default: 7d.
pub const MIN_IRREVERSIBLE_TIMELOCK_SEC: i64 = 7 * 24 * 60 * 60;

/// Fixed at compile time. For **mainnet** with different approvers, deploy a build whose array matches your ops keys;
/// keep `admin-final.html` `UPGRADE_SIGNERS_MAINNET` in sync for UI labels / “Check wallet”.
pub const UPGRADE_SIGNERS: [Pubkey; 5] = [
    pubkey!("5fkgfLSGCxJTWcqQHfzigQUnxA1NAaCmmCjQbXmTvVzc"),
    pubkey!("CwyNHESJ95mccZkGPEEApQdeB4XEV5mSL1SRkn6Ee8qG"),
    pubkey!("8TeEjQkh2CQTbKo57r3n5GrYGYUzvrmbj1eRJgbjZsjp"),
    pubkey!("BpDZ6jrcPYo1GoM4DWk857ys4R7MgyZb4FmHjkC9beuH"),
    pubkey!("CQsV3Wj6pdcgEkk5hkS6bd31Q2xp9fCuAqvV9WoLjqAR"),
];

fn is_upgrade_signer(k: &Pubkey) -> bool {
    UPGRADE_SIGNERS.iter().any(|pk| pk == k)
}

fn require_upgrade_signer(k: &Pubkey) -> Result<()> {
    require!(is_upgrade_signer(k), ErrorCode::Unauthorized);
    Ok(())
}

#[error_code]
pub enum ErrorCode {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Timelock not yet passed")]
    TimelockNotPassed,
    #[msg("Timelock must be 0 or at least 24 hours (execute-upgrade policy)")]
    InvalidTimelock,
    #[msg("Upgrades are currently paused")]
    UpgradesPaused,
    #[msg("Irreversible timelock must be at least 7 days")]
    IrevMinTimelock,
    #[msg("Need 5/5 on irreversible approvals to execute close")]
    IrevNeedFive,
    #[msg("Irreversible close timelock not passed")]
    IrevTimelockNotPassed,
    #[msg("Proposed program id is all zeros; call propose_close_program first")]
    IrevNotProposed,
    #[msg("Cannot close the governor program with this instruction")]
    CannotCloseGovernor,
}

#[program]
pub mod upgrade_governor {
    use super::*;

    pub fn init_upgrade_gate(ctx: Context<InitUpgradeGate>) -> Result<()> {
        let gate = &mut ctx.accounts.upgrade_gate;
        gate.approvals = 0;
        gate.last_reset = Clock::get()?.unix_timestamp;
        gate.bump = ctx.bumps.upgrade_gate;
        gate.timelock_duration = 0;
        gate.queued_at = 0;
        gate.paused = false;
        msg!("Governor: upgrade gate initialized");
        Ok(())
    }

    /// One of 5; sets a bit. At 3/5 and first queue, `queued_at` is set. Use **Set timelock** (with 3+ bits) before
    /// 3/5 to enforce a post-queue delay on **execute** (or set 0 to allow immediate run after 3/5 if no timelock set).
    pub fn approve_upgrade(ctx: Context<ApproveUpgrade>, signer_index: u8) -> Result<()> {
        require!(signer_index < 5, ErrorCode::Unauthorized);
        let signer_pubkey = ctx.accounts.signer.key();
        require_keys_eq!(
            signer_pubkey,
            UPGRADE_SIGNERS[signer_index as usize],
            ErrorCode::Unauthorized
        );
        let gate = &mut ctx.accounts.upgrade_gate;
        gate.approvals |= 1 << signer_index;
        gate.last_reset = Clock::get()?.unix_timestamp;
        if gate.approvals.count_ones() >= 3 && gate.queued_at == 0 {
            gate.queued_at = Clock::get()?.unix_timestamp;
            emit!(UpgradeQueued {
                queued_at: gate.queued_at,
                timelock_duration: gate.timelock_duration,
            });
            msg!("Governor: 3 approvals — upgrade queued");
        }
        Ok(())
    }

    /// Requires **3/5 approval bits** and **tx signer in UPGRADE_SIGNERS**. Resets `timelock_duration` (execute delay).
    pub fn set_timelock(ctx: Context<SetTimelock>, new_duration: i64) -> Result<()> {
        require_upgrade_signer(&ctx.accounts.signer.key())?;
        let gate = &mut ctx.accounts.upgrade_gate;
        require!(gate.approvals.count_ones() >= 3, ErrorCode::Unauthorized);
        if new_duration != 0 && new_duration < 86400 {
            return err!(ErrorCode::InvalidTimelock);
        }
        gate.timelock_duration = new_duration;
        gate.approvals = 0;
        gate.queued_at = 0;
        Ok(())
    }

    /// Normal pause: same policy as set_timelock: **3/5 on record** and **signed by one of UPGRADE_SIGNERS**.
    pub fn pause_upgrades(ctx: Context<PauseUpgrades>, pause: bool) -> Result<()> {
        require_upgrade_signer(&ctx.accounts.signer.key())?;
        let gate = &mut ctx.accounts.upgrade_gate;
        require!(gate.approvals.count_ones() >= 3, ErrorCode::Unauthorized);
        gate.paused = pause;
        gate.approvals = 0;
        gate.queued_at = 0;
        Ok(())
    }

    pub fn execute_upgrade(
        ctx: Context<ExecuteUpgrade>,
        _buffer: Pubkey,
        _target_program: Pubkey,
    ) -> Result<()> {
        let caller = ctx.accounts.authority.key();

        require_keys_eq!(
            ctx.accounts.buffer.key(),
            _buffer,
            ErrorCode::Unauthorized
        );
        require_keys_eq!(
            ctx.accounts.target_program.key(),
            _target_program,
            ErrorCode::Unauthorized
        );

        require!(
            ctx.accounts.upgrade_gate.approvals.count_ones() >= 3,
            ErrorCode::Unauthorized
        );
        require!(!ctx.accounts.upgrade_gate.paused, ErrorCode::UpgradesPaused);

        if ctx.accounts.upgrade_gate.timelock_duration > 0 {
            let now = Clock::get()?.unix_timestamp;
            require!(
                now
                    >= ctx.accounts.upgrade_gate.queued_at
                        + ctx.accounts.upgrade_gate.timelock_duration,
                ErrorCode::TimelockNotPassed
            );
        }

        let is_one_of_five = is_upgrade_signer(&caller);
        require!(is_one_of_five, ErrorCode::Unauthorized);

        let (gate_key, gate_bump) = {
            let gate = &ctx.accounts.upgrade_gate;
            (gate.key(), gate.bump)
        };

        let gate = &mut ctx.accounts.upgrade_gate;
        let gate_info = gate.to_account_info();

        let upgrade_ix = bpf_loader_upgradeable::upgrade(
            &ctx.accounts.target_program.key(),
            &ctx.accounts.buffer.key(),
            &gate_key,
            &ctx.accounts.spill.key(),
        );

        invoke_signed(
            &upgrade_ix,
            &[
                ctx.accounts.program_data.to_account_info(),
                ctx.accounts.target_program.to_account_info(),
                ctx.accounts.buffer.to_account_info(),
                ctx.accounts.spill.to_account_info(),
                ctx.accounts.rent.to_account_info(),
                ctx.accounts.clock.to_account_info(),
                gate_info,
            ],
            &[&[b"upgrade-gate", &[gate_bump]]],
        )?;

        emit!(UpgradeExecuted {
            buffer: _buffer,
            slot: Clock::get()?.slot,
            authority: caller,
        });

        gate.approvals = 0;
        gate.queued_at = 0;
        msg!("Governor: upgrade executed");
        Ok(())
    }

    pub fn withdraw_sol(ctx: Context<WithdrawSol>, amount: u64) -> Result<()> {
        require_upgrade_signer(&ctx.accounts.authority.key())?;
        let gate = &ctx.accounts.upgrade_gate;
        require!(gate.approvals.count_ones() >= 3, ErrorCode::Unauthorized);
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.key(),
                system_program::Transfer {
                    from: ctx.accounts.funds_account.to_account_info(),
                    to: ctx.accounts.recipient.to_account_info(),
                },
            ),
            amount,
        )?;
        Ok(())
    }

    // --- 5/5 + timelock: close upgradeable program (CPI) ---

    /// Proposes which program to close, rent recipient, and the **extra** wait after 5/5. **Signer** must be in
    /// `UPGRADE_SIGNERS`. `timelock_after_five` must be at least 7d (on-chain `MIN_IRREVERSIBLE_TIMELOCK_SEC`).
    pub fn propose_close_program(
        ctx: Context<ProposeCloseProgram>,
        program_to_close: Pubkey,
        close_rent_recipient: Pubkey,
        timelock_after_five: i64,
    ) -> Result<()> {
        require_upgrade_signer(&ctx.accounts.signer.key())?;
        require!(program_to_close != crate::id(), ErrorCode::CannotCloseGovernor);
        if timelock_after_five != 0 && timelock_after_five < MIN_IRREVERSIBLE_TIMELOCK_SEC {
            return err!(ErrorCode::IrevMinTimelock);
        }
        let e = &mut ctx.accounts.irreversible;
        e.bump = ctx.bumps.irreversible;
        e.approvals = 0;
        e.queued_at = 0;
        e.timelock_after_five = timelock_after_five;
        e.program_to_close = program_to_close;
        e.close_rent_recipient = close_rent_recipient;
        msg!("Governor: proposed close; need 5/5 approvers then timelock");
        Ok(())
    }

    /// Bit **separate** from `upgrade_gate.approvals`. On 5/5, `queued_at` is set for the irreversible timelock.
    pub fn approve_irreversible(ctx: Context<ApproveIrreversible>, signer_index: u8) -> Result<()> {
        require!(signer_index < 5, ErrorCode::Unauthorized);
        require_keys_eq!(
            ctx.accounts.signer.key(),
            UPGRADE_SIGNERS[signer_index as usize],
            ErrorCode::Unauthorized
        );
        require!(
            ctx.accounts.irreversible.program_to_close != Pubkey::default(),
            ErrorCode::IrevNotProposed
        );
        let e = &mut ctx.accounts.irreversible;
        e.approvals |= 1 << signer_index;
        if e.approvals.count_ones() >= 5 && e.queued_at == 0 {
            e.queued_at = Clock::get()?.unix_timestamp;
            emit!(IrreversibleCloseQueued {
                at: e.queued_at,
                after_five_sec: e.timelock_after_five,
            });
            msg!("Governor: 5/5 for close — wait timelock then execute");
        }
        Ok(())
    }

    /// PDA (upgrade gate) signs the loader as **current upgrade authority**; **authority** (one of five) is the
    /// trigger + fee payer. Destroys the program; cannot target this governor.
    pub fn execute_close_program(ctx: Context<ExecuteCloseProgram>) -> Result<()> {
        require_upgrade_signer(&ctx.accounts.authority.key())?;
        require!(
            ctx.accounts.irreversible.program_to_close != Pubkey::default(),
            ErrorCode::IrevNotProposed
        );
        require!(
            ctx.accounts.irreversible.approvals.count_ones() >= 5,
            ErrorCode::IrevNeedFive
        );
        let ir = &ctx.accounts.irreversible;
        if ir.timelock_after_five > 0 {
            let now = Clock::get()?.unix_timestamp;
            require!(
                now >= ir.queued_at + ir.timelock_after_five,
                ErrorCode::IrevTimelockNotPassed
            );
        }

        let gate_key = ctx.accounts.upgrade_gate.key();
        let program = ctx.accounts.program_to_close.key();
        let recipient = ctx.accounts.close_rent_recipient.key();
        let program_data = ctx.accounts.program_data_for_close.key();
        require_keys_eq!(program, ir.program_to_close, ErrorCode::Unauthorized);
        require_keys_eq!(recipient, ir.close_rent_recipient, ErrorCode::Unauthorized);

        let (pd_expected, _b) =
            Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::id());
        require_keys_eq!(program_data, pd_expected, ErrorCode::Unauthorized);
        require!(program != *ctx.program_id, ErrorCode::CannotCloseGovernor);

        let gate_bump = ctx.accounts.upgrade_gate.bump;
        let close_ix = close_any(
            &program_data,
            &recipient,
            Some(&gate_key),
            Some(&program),
        );
        let gate_info = ctx.accounts.upgrade_gate.to_account_info();

        invoke_signed(
            &close_ix,
            &[
                ctx.accounts.program_data_for_close.to_account_info(),
                ctx.accounts.close_rent_recipient.to_account_info(),
                gate_info,
                ctx.accounts.program_to_close.to_account_info(),
            ],
            &[&[b"upgrade-gate", &[gate_bump]]],
        )?;

        let e = &mut ctx.accounts.irreversible;
        e.approvals = 0;
        e.queued_at = 0;
        e.program_to_close = Pubkey::default();
        e.close_rent_recipient = Pubkey::default();
        e.timelock_after_five = 0;
        msg!("Governor: program closed (irreversible path)");
        Ok(())
    }
}

#[event]
pub struct UpgradeQueued {
    pub queued_at: i64,
    pub timelock_duration: i64,
}

#[event]
pub struct UpgradeExecuted {
    pub buffer: Pubkey,
    pub slot: u64,
    pub authority: Pubkey,
}

#[event]
pub struct IrreversibleCloseQueued {
    pub at: i64,
    pub after_five_sec: i64,
}

#[account]
pub struct UpgradeGate {
    pub approvals: u8,
    pub last_reset: i64,
    pub bump: u8,
    pub timelock_duration: i64,
    pub queued_at: i64,
    pub paused: bool,
}

/// Single pending 5/5 + timelock for **one** `close` target at a time (re-propose to replace).
#[account]
pub struct IrreversibleClose {
    pub bump: u8,
    /// Bitmask of 5 signers; must be 5/5 to execute
    pub approvals: u8,
    pub queued_at: i64,
    /// After 5/5, wait this many seconds before `execute_close_program` (min 0 only if =0 with care; use ≥7d prod)
    pub timelock_after_five: i64,
    pub program_to_close: Pubkey,
    pub close_rent_recipient: Pubkey,
}

#[derive(Accounts)]
pub struct InitUpgradeGate<'info> {
    #[account(init, payer = user, space = 8 + 1 + 8 + 1 + 8 + 8 + 1, seeds = [b"upgrade-gate"], bump)]
    pub upgrade_gate: Account<'info, UpgradeGate>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ApproveUpgrade<'info> {
    #[account(mut, seeds = [b"upgrade-gate"], bump = upgrade_gate.bump)]
    pub upgrade_gate: Account<'info, UpgradeGate>,
    pub signer: Signer<'info>,
}

#[derive(Accounts)]
pub struct SetTimelock<'info> {
    #[account(mut, seeds = [b"upgrade-gate"], bump = upgrade_gate.bump)]
    pub upgrade_gate: Account<'info, UpgradeGate>,
    pub signer: Signer<'info>,
}

#[derive(Accounts)]
pub struct PauseUpgrades<'info> {
    #[account(mut, seeds = [b"upgrade-gate"], bump = upgrade_gate.bump)]
    pub upgrade_gate: Account<'info, UpgradeGate>,
    pub signer: Signer<'info>,
}

#[derive(Accounts)]
pub struct ExecuteUpgrade<'info> {
    #[account(mut, seeds = [b"upgrade-gate"], bump = upgrade_gate.bump)]
    pub upgrade_gate: Account<'info, UpgradeGate>,
    pub authority: Signer<'info>,
    /// CHECK: Buffer account for program upgrade
    #[account(mut)]
    pub buffer: UncheckedAccount<'info>,
    /// CHECK: Target program being upgraded (must differ from this governor program)
    #[account(mut)]
    pub target_program: UncheckedAccount<'info>,
    /// CHECK: ProgramData PDA for the target program
    #[account(
        mut,
        seeds = [target_program.key().as_ref()],
        bump,
        seeds::program = bpf_loader_upgradeable::id()
    )]
    pub program_data: UncheckedAccount<'info>,
    /// CHECK: Recipient for reclaimed lamports from closed buffer
    #[account(mut)]
    pub spill: UncheckedAccount<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub clock: Sysvar<'info, Clock>,
    /// CHECK: BPF upgradeable loader program id
    #[account(address = bpf_loader_upgradeable::id())]
    pub bpf_loader: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct WithdrawSol<'info> {
    #[account(seeds = [b"upgrade-gate"], bump)]
    pub upgrade_gate: Account<'info, UpgradeGate>,
    #[account(mut)]
    pub funds_account: Signer<'info>,
    #[account(mut)]
    pub recipient: SystemAccount<'info>,
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ProposeCloseProgram<'info> {
    /// CHECK: global single proposal slot
    #[account(
        init_if_needed,
        payer = signer,
        space = 8 + 1 + 1 + 8 + 8 + 32 + 32,
        seeds = [b"irreversible"],
        bump
    )]
    pub irreversible: Account<'info, IrreversibleClose>,
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ApproveIrreversible<'info> {
    #[account(mut, seeds = [b"irreversible"], bump = irreversible.bump)]
    pub irreversible: Account<'info, IrreversibleClose>,
    pub signer: Signer<'info>,
}

#[derive(Accounts)]
pub struct ExecuteCloseProgram<'info> {
    #[account(seeds = [b"upgrade-gate"], bump = upgrade_gate.bump, mut)]
    pub upgrade_gate: Account<'info, UpgradeGate>,
    /// CHECK: must be upgradeable program id to close; not governor
    #[account(mut, owner = bpf_loader_upgradeable::id())]
    pub program_to_close: UncheckedAccount<'info>,
    /// CHECK: ProgramData PDA for that program; matches seeds
    #[account(
        mut,
        seeds = [program_to_close.key().as_ref()],
        bump,
        seeds::program = bpf_loader_upgradeable::id()
    )]
    pub program_data_for_close: UncheckedAccount<'info>,
    /// CHECK: receives ProgramData+program rent
    #[account(mut)]
    pub close_rent_recipient: UncheckedAccount<'info>,
    /// CHECK: PDA in [`IrreversibleClose`]
    #[account(mut, seeds = [b"irreversible"], bump = irreversible.bump)]
    pub irreversible: Account<'info, IrreversibleClose>,
    pub authority: Signer<'info>,
    /// CHECK: required in tx for upgradeable program close CPI
    #[account(address = bpf_loader_upgradeable::id())]
    pub bpf_loader: UncheckedAccount<'info>,
}
