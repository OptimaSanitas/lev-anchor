//! Minimal on-chain program: 3-of-5 multisig gate for BPF upgradeable upgrades.
//! Deploy this program, initialize its gate PDA, then set **another** program's
//! upgrade authority to that PDA. `execute_upgrade` CPIs the loader for the target.

#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_lang::solana_program::{bpf_loader_upgradeable, program::invoke_signed};

declare_id!("73yx8W1HB7kooLXSgoqJtNoBNbRUKaotFNygd7b9dDRQ");

pub const UPGRADE_SIGNERS: [Pubkey; 5] = [
    pubkey!("5fkgfLSGCxJTWcqQHfzigQUnxA1NAaCmmCjQbXmTvVzc"),
    pubkey!("CwyNHESJ95mccZkGPEEApQdeB4XEV5mSL1SRkn6Ee8qG"),
    pubkey!("8TeEjQkh2CQTbKo57r3n5GrYGYUzvrmbj1eRJgbjZsjp"),
    pubkey!("BpDZ6jrcPYo1GoM4DWk857ys4R7MgyZb4FmHjkC9beuH"),
    pubkey!("CQsV3Wj6pdcgEkk5hkS6bd31Q2xp9fCuAqvV9WoLjqAR"),
];

#[error_code]
pub enum ErrorCode {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Timelock not yet passed")]
    TimelockNotPassed,
    #[msg("Timelock must be 0 or at least 24 hours")]
    InvalidTimelock,
    #[msg("Upgrades are currently paused")]
    UpgradesPaused,
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

    pub fn set_timelock(ctx: Context<SetTimelock>, new_duration: i64) -> Result<()> {
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

    pub fn pause_upgrades(ctx: Context<PauseUpgrades>, pause: bool) -> Result<()> {
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
                now >= ctx.accounts.upgrade_gate.queued_at + ctx.accounts.upgrade_gate.timelock_duration,
                ErrorCode::TimelockNotPassed
            );
        }

        let is_one_of_five = UPGRADE_SIGNERS.iter().any(|&pk| pk == caller);
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

#[account]
pub struct UpgradeGate {
    pub approvals: u8,
    pub last_reset: i64,
    pub bump: u8,
    pub timelock_duration: i64,
    pub queued_at: i64,
    pub paused: bool,
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
        seeds::program = bpf_loader_upgradeable::ID
    )]
    pub program_data: UncheckedAccount<'info>,
    /// CHECK: Recipient for reclaimed lamports from closed buffer
    #[account(mut)]
    pub spill: UncheckedAccount<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub clock: Sysvar<'info, Clock>,
    /// CHECK: BPF Upgradeable Loader program id must be listed on this instruction so CPI to it is allowed.
    #[account(address = bpf_loader_upgradeable::ID)]
    pub bpf_loader: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct WithdrawSol<'info> {
    #[account(mut, seeds = [b"upgrade-gate"], bump)]
    pub upgrade_gate: Account<'info, UpgradeGate>,
    #[account(mut)]
    pub funds_account: Signer<'info>,
    #[account(mut)]
    pub recipient: SystemAccount<'info>,
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}
