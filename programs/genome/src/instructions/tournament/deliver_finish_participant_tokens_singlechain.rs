use crate::{
    error::TournamentError, OperatorInfo, Role, Team, Tournament, TournamentStatus, BP_DEC, GENOME_ROOT,
};
use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::TransferChecked,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface},
};

#[derive(Accounts)]
pub struct DeliverParticipantTokensSinglechain<'info> {
    #[account(signer, mut)]
    pub admin: Signer<'info>,
    #[account(seeds = [GENOME_ROOT, b"OPERATOR", admin.key().as_ref()], bump, constraint = operator_info.approved && (operator_info.role == Role::BACKEND || operator_info.role == Role::OWNER) @ TournamentError::OperatorNotApprovedOrInvalidRole)]
    pub operator_info: Box<Account<'info, OperatorInfo>>,
    #[account(
        mut,
        seeds = [GENOME_ROOT, b"TOURNAMENT", &tournament.id.to_le_bytes().as_ref()],
        bump
    )]
    pub tournament: Box<Account<'info, Tournament>>,
    #[account(mut)]
    pub team: Box<Account<'info, Team>>,
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut)]
    pub participant: AccountInfo<'info>,
    #[account(
        mut,
        associated_token::mint = mint, 
        associated_token::authority = participant,
        associated_token::token_program = token_program,
    )]
    pub participant_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint, 
        associated_token::authority = tournament,
        associated_token::token_program = token_program,
    )]
    pub tournament_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handle_deliver_participant_tokens_singlechain(
    ctx: Context<DeliverParticipantTokensSinglechain>
) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let team = &mut ctx.accounts.team;
    if tournament.status != TournamentStatus::PreFinish {
        return Err(TournamentError::InvalidTournamentStatus.into());
    }

    if team.players.len() != tournament.players_in_team as usize {
        return Err(TournamentError::NotAllTeamsParticipated.into());
    }

    if tournament.finish_metadata.winners.contains(&team.key()) {
        return Err(TournamentError::NotWinner.into());
    }

    // note: one thing is the index of captain from tournament.captains array
    // and other thing is the winners array, which can have captains in an unordered way
    let captain_index_in_winners_vec = tournament
        .finish_metadata
        .winners
        .iter()
        .position(|&x| x == team.captain);
    if captain_index_in_winners_vec.is_none() {
        return Err(TournamentError::TeamNotFound.into());
    }
    let captain_index_in_winners_vec = captain_index_in_winners_vec.unwrap();
    if tournament.finish_metadata.rewarded_winners[captain_index_in_winners_vec] {
        return Err(TournamentError::AlreadyPaid.into());
    }

    let participant_in_team_index = team.players.iter().position(|&x| x == ctx.accounts.participant.key());
    if participant_in_team_index.is_none() {
        return Err(TournamentError::ParticipantNotFound.into());
    }
    let participant_in_team_index = participant_in_team_index.unwrap();
    if team.players_money_delivered[participant_in_team_index] {
        return Err(TournamentError::AlreadyPaid.into());
    }
    team.players_money_delivered[participant_in_team_index] = true;

    let reward = (tournament.finish_metadata.total_prize_pool
        * tournament.finish_metadata.rewards_prize_fractions[captain_index_in_winners_vec] as u64)
        / (BP_DEC as u64 * tournament.players_in_team as u64);
        
    let seeds = &[
        GENOME_ROOT,
        b"TOURNAMENT",
        &tournament.id.to_le_bytes()[..],
        &[tournament.bump],
    ];
    let signer_seeds = [&seeds[..]];

    let accounts = TransferChecked {
        from: ctx.accounts.tournament_vault.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.participant_vault.to_account_info(),
        authority: tournament.to_account_info(),
    };

    let cpi_context = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        accounts,
        &signer_seeds,
    );

    transfer_checked(cpi_context, reward, ctx.accounts.mint.decimals)?;
    tournament.finish_metadata.remaining_prize_pool -= reward;


    if team.players_money_delivered.iter().all(|&x| x) {
        tournament.finish_metadata.rewarded_winners[captain_index_in_winners_vec] = true;
    }

    Ok(())
}