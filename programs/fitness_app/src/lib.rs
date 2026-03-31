use anchor_lang::prelude::*;

declare_id!("BUPY7yPt6BqWUTHmqLteEfRbH9zH8zQMcUNA9NRBFYEz");

#[program]
pub mod fitness_app {
    use super::*;

    // Initialize global counter
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.counter.count = 0;
        Ok(())
    }

    // Mint SBT - Simplified phased system (1000 per version)
    pub fn mint_sbt(ctx: Context<MintSbt>, version: u8, uri: String) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        let sbt = &mut ctx.accounts.sbt;

        counter.count = counter.count.checked_add(1).unwrap();

        require!(version >= 1 && version <= 10, ErrorCode::InvalidVersion);

        // Simple 1000 per version logic
        let phase_start = ((version as u64) - 1) * 1000 + 1;
        let phase_end = version as u64 * 1000;

        require!(counter.count >= phase_start && counter.count <= phase_end, ErrorCode::PhaseNotReached);

        sbt.owner = ctx.accounts.user.key();
        sbt.uri = uri;
        sbt.version = version;
        sbt.is_early = version == 1;
        sbt.total_sets_completed = 0;
        sbt.total_distance_walked = 0;
        sbt.total_distance_ran = 0;
        sbt.encrypted_fitness_data = vec![];
        sbt.last_updated = Clock::get()?.unix_timestamp;
        sbt.data_version = 1;
        sbt.bump = ctx.bumps.sbt;

        Ok(())
    }

    // Update public fitness stats (use this now)
    pub fn update_fitness_stats(ctx: Context<UpdateFitnessStats>, walked: u64, ran: u64, sets: u32) -> Result<()> {
        let sbt = &mut ctx.accounts.sbt;
        require_keys_eq!(sbt.owner, ctx.accounts.user.key(), ErrorCode::Unauthorized);

        sbt.total_distance_walked = sbt.total_distance_walked.checked_add(walked).unwrap_or(sbt.total_distance_walked);
        sbt.total_distance_ran = sbt.total_distance_ran.checked_add(ran).unwrap_or(sbt.total_distance_ran);
        sbt.total_sets_completed = sbt.total_sets_completed.checked_add(sets).unwrap_or(sbt.total_sets_completed);
        sbt.last_updated = Clock::get()?.unix_timestamp;

        Ok(())
    }

    // Update encrypted fitness data (for MagicBlock later)
    pub fn update_encrypted_fitness(ctx: Context<UpdateEncryptedFitness>, encrypted_data: Vec<u8>) -> Result<()> {
        let sbt = &mut ctx.accounts.sbt;
        require_keys_eq!(sbt.owner, ctx.accounts.user.key(), ErrorCode::Unauthorized);

        sbt.encrypted_fitness_data = encrypted_data;
        sbt.last_updated = Clock::get()?.unix_timestamp;

        Ok(())
    }

    // Update public URI (for image/metadata changes)
    pub fn update_sbt_uri(ctx: Context<UpdateSbtUri>, new_uri: String) -> Result<()> {
        let sbt = &mut ctx.accounts.sbt;
        require_keys_eq!(sbt.owner, ctx.accounts.user.key(), ErrorCode::Unauthorized);
        sbt.uri = new_uri;
        Ok(())
    }
}

// ====================== ACCOUNTS ======================

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = user, space = 8 + 8, seeds = [b"counter"], bump)]
    pub counter: Account<'info, Counter>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(version: u8)]
pub struct MintSbt<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut, seeds = [b"counter"], bump)]
    pub counter: Account<'info, Counter>,

    #[account(
        init,
        payer = user,
        space = 8 + 32 + 4 + 200 + 1 + 1 + 8 + 8 + 8 + 4 + 1 + 8 + 1, // updated space
        seeds = [user.key().as_ref(), b"sbt", &[version]],
        bump
    )]
    pub sbt: Account<'info, SbtAccount>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateFitnessStats<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [user.key().as_ref(), b"sbt", &[sbt.version]],
        bump = sbt.bump
    )]
    pub sbt: Account<'info, SbtAccount>,
}

#[derive(Accounts)]
pub struct UpdateEncryptedFitness<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [user.key().as_ref(), b"sbt", &[sbt.version]],
        bump = sbt.bump
    )]
    pub sbt: Account<'info, SbtAccount>,
}

#[derive(Accounts)]
pub struct UpdateSbtUri<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [user.key().as_ref(), b"sbt", &[sbt.version]],
        bump = sbt.bump
    )]
    pub sbt: Account<'info, SbtAccount>,
}

// ====================== DATA STRUCTURES ======================

#[account]
pub struct Counter {
    pub count: u64,
}

#[account]
pub struct SbtAccount {
    pub owner: Pubkey,
    pub uri: String,
    pub version: u8,
    pub is_early: bool,
    pub bump: u8,

    // Public fitness stats
    pub total_sets_completed: u32,
    pub total_distance_walked: u64,
    pub total_distance_ran: u64,

    // Future encrypted data (MagicBlock)
    pub encrypted_fitness_data: Vec<u8>,
 use anchor_lang::prelude::*;

declare_id!("BUPY7yPt6BqWUTHmqLteEfRbH9zH8zQMcUNA9NRBFYEz");

#[program]
pub mod fitness_app {
    use super::*;

    // Initialize global counter
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.counter.count = 0;
        Ok(())
    }

    // Mint SBT - Simplified phased system (1000 per version)
    pub fn mint_sbt(ctx: Context<MintSbt>, version: u8, uri: String) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        let sbt = &mut ctx.accounts.sbt;

        counter.count = counter.count.checked_add(1).unwrap();

        require!(version >= 1 && version <= 10, ErrorCode::InvalidVersion);

        // Simple 1000 per version logic
        let phase_start = ((version as u64) - 1) * 1000 + 1;
        let phase_end = version as u64 * 1000;

        require!(counter.count >= phase_start && counter.count <= phase_end, ErrorCode::PhaseNotReached);

        sbt.owner = ctx.accounts.user.key();
        sbt.uri = uri;
        sbt.version = version;
        sbt.is_early = version == 1;
        sbt.total_sets_completed = 0;
        sbt.total_distance_walked = 0;
        sbt.total_distance_ran = 0;
        sbt.encrypted_fitness_data = vec![];
        sbt.last_updated = Clock::get()?.unix_timestamp;
        sbt.data_version = 1;
        sbt.bump = ctx.bumps.sbt;

        Ok(())
    }

    // Update public fitness stats (use this now)
    pub fn update_fitness_stats(ctx: Context<UpdateFitnessStats>, walked: u64, ran: u64, sets: u32) -> Result<()> {
        let sbt = &mut ctx.accounts.sbt;
        require_keys_eq!(sbt.owner, ctx.accounts.user.key(), ErrorCode::Unauthorized);

        sbt.total_distance_walked = sbt.total_distance_walked.checked_add(walked).unwrap_or(sbt.total_distance_walked);
        sbt.total_distance_ran = sbt.total_distance_ran.checked_add(ran).unwrap_or(sbt.total_distance_ran);
        sbt.total_sets_completed = sbt.total_sets_completed.checked_add(sets).unwrap_or(sbt.total_sets_completed);
        sbt.last_updated = Clock::get()?.unix_timestamp;

        Ok(())
    }

    // Update encrypted fitness data (for MagicBlock later)
    pub fn update_encrypted_fitness(ctx: Context<UpdateEncryptedFitness>, encrypted_data: Vec<u8>) -> Result<()> {
        let sbt = &mut ctx.accounts.sbt;
        require_keys_eq!(sbt.owner, ctx.accounts.user.key(), ErrorCode::Unauthorized);

        sbt.encrypted_fitness_data = encrypted_data;
        sbt.last_updated = Clock::get()?.unix_timestamp;

        Ok(())
    }

    // Update public URI (for image/metadata changes)
    pub fn update_sbt_uri(ctx: Context<UpdateSbtUri>, new_uri: String) -> Result<()> {
        let sbt = &mut ctx.accounts.sbt;
        require_keys_eq!(sbt.owner, ctx.accounts.user.key(), ErrorCode::Unauthorized);
        sbt.uri = new_uri;
        Ok(())
    }
}

// ====================== ACCOUNTS ======================

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = user, space = 8 + 8, seeds = [b"counter"], bump)]
    pub counter: Account<'info, Counter>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(version: u8)]
pub struct MintSbt<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut, seeds = [b"counter"], bump)]
    pub counter: Account<'info, Counter>,

    #[account(
        init,
        payer = user,
        space = 8 + 32 + 4 + 200 + 1 + 1 + 8 + 8 + 8 + 4 + 1 + 8 + 1, // updated space
        seeds = [user.key().as_ref(), b"sbt", &[version]],
        bump
    )]
    pub sbt: Account<'info, SbtAccount>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateFitnessStats<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [user.key().as_ref(), b"sbt", &[sbt.version]],
        bump = sbt.bump
    )]
    pub sbt: Account<'info, SbtAccount>,
}

#[derive(Accounts)]
pub struct UpdateEncryptedFitness<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [user.key().as_ref(), b"sbt", &[sbt.version]],
        bump = sbt.bump
    )]
    pub sbt: Account<'info, SbtAccount>,
}

#[derive(Accounts)]
pub struct UpdateSbtUri<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [user.key().as_ref(), b"sbt", &[sbt.version]],
        bump = sbt.bump
    )]
    pub sbt: Account<'info, SbtAccount>,
}

// ====================== DATA STRUCTURES ======================

#[account]
pub struct Counter {
    pub count: u64,
}

#[account]
pub struct SbtAccount {
    pub owner: Pubkey,
    pub uri: String,
    pub version: u8,
    pub is_early: bool,
    pub bump: u8,

    // Public fitness stats
    pub total_sets_completed: u32,
    pub total_distance_walked: u64,
    pub total_distance_ran: u64,

    // Future encrypted data (MagicBlock)
    pub encrypted_fitness_data: Vec<u8>,
    pub last_updated: i64,
    pub data_version: u8,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid SBT version")]
    InvalidVersion,
    #[msg("This phase has not started yet or is already full")]
    PhaseNotReached,
    #[msg("Unauthorized: Only the owner can update this SBT")]
    Unauthorized,
}   pub last_updated: i64,
    pub data_version: u8,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid SBT version")]
    InvalidVersion,
    #[msg("This phase has not started yet or is already full")]
    PhaseNotReached,
    #[msg("Unauthorized: Only the owner can update this SBT")]
    Unauthorized,
}
