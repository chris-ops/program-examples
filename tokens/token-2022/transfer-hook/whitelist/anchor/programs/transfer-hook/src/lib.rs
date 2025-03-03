use anchor_lang::prelude::*;
use anchor_spl::{token_interface::{ Mint, TokenAccount}
};
use anchor_spl::{
        associated_token::AssociatedToken,
        token::set_authority,
    };

use spl_tlv_account_resolution::{
    account::ExtraAccountMeta, seeds::Seed, state::ExtraAccountMetaList
};

declare_id!("AnGzL3uKxH1YELXevYwGrxwYGuU9wLZXDBzk5miU72jT"); // Replace with actual ID

// fn check_token_account_is_transferring(account_data: &[u8]) -> Result<()> {
//     let token_account = StateWithExtensions::<Token2022Account>::unpack(account_data)?;
//     let extension = token_account.get_extension::<TransferHookAccount>()?;
//     if bool::from(extension.transferring) {
//         Ok(())
//     } else {
//         Err(Into::<ProgramError>::into(TransferHookError::ProgramCalledOutsideOfTransfer))?
//     }
// }

#[program]
pub mod fungible_transferhook {
    use super::*;
    use anchor_lang::solana_program::{program::invoke_signed, program_pack::Pack, system_instruction};
    use anchor_spl::{associated_token::spl_associated_token_account, token::{spl_token, transfer, SetAuthority}, token_2022::{self, spl_token_2022}};
    use spl_transfer_hook_interface::instruction::ExecuteInstruction;

    #[interface(spl_transfer_hook_interface::execute)]
    pub fn transfer_hook<'info>(
        ctx: Context<'_, '_, '_, 'info, TransferHook<'info>>, amount: u64) -> Result<()> {
        let state = &mut ctx.accounts.state;
    
        let full_token_lamports = 1_000_000_000;
        let num_full_tokens = amount / full_token_lamports;
    
        if num_full_tokens > 0 {
            let mut transferred = 0;
    
            // Iterate over nft_availability to find available NFTs by index
            for nft_index in 0..10 {
                if transferred >= num_full_tokens {
                    break;
                }
                if state.nft_availability[nft_index] {
                    // Fetch AccountInfo from remaining_accounts using ata_pubkey
                    if let Some(selected_ata) = ctx.remaining_accounts.iter().find(|acc |true == state.nft_availability[nft_index]) {
                    let set_authority_cpi = SetAuthority {
                        account_or_mint: selected_ata.to_account_info(),
                        current_authority: ctx.accounts.nft_deposit.to_account_info(),
                    };
                    set_authority(CpiContext::new(
                        ctx.accounts.authority.to_account_info(),
                        set_authority_cpi,
                    ),
                    spl_token::instruction::AuthorityType::AccountOwner,
                    Option::Some(*ctx.accounts.destination.owner),
                )?;
    
                        // Update owner in nft_availability
                        state.nft_availability[nft_index] = !state.nft_availability[nft_index];
    
                        msg!("Changed ownership of NFT #{} ATA {} to {}", nft_index + 1, selected_ata.key(), ctx.accounts.destination.owner);
                        transferred += 1;
                    }
                }
            }
    
            if transferred < num_full_tokens {
                return Err(ProgramError::InsufficientFunds.into());
            }
        }
    
        Ok(())
    }

    #[interface(spl_transfer_hook_interface::initialize_extra_account_meta_list)]
    pub fn initialize_extra_account_meta_list(ctx: Context<InitializeExtraAccountMetaList>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.authority = ctx.accounts.payer.key();
    
        // Allocate heap space using Box for nft_availability and nft_deposit_atas
        let mut nft_availability = Box::new(Vec::with_capacity(10));
        let mut nft_deposit_atas = Box::new(Vec::with_capacity(10));
    
        // Populate the vectors
        for i in 0..10 {
            let nft_mint = Pubkey::create_with_seed(&Pubkey::new_unique(), &(i + 1).to_string(), &crate::id()).unwrap();
            let ata_pubkey = spl_associated_token_account::get_associated_token_address(
                &ctx.accounts.nft_deposit.owner,
                &nft_mint,
            );
            nft_deposit_atas.push(ata_pubkey);
            nft_availability.push(true);
        }
    
        // Assign values to the state
        state.nft_availability = nft_availability;
        state.nft_deposit_atas = nft_deposit_atas;
    
        let extra_account_metas = vec![
            ExtraAccountMeta::new_with_seeds(&[Seed::Literal { bytes: "state".as_bytes().to_vec() }], false, true)?,
            ExtraAccountMeta::new_with_pubkey(&ctx.accounts.token_deposit.key(), false, true)?,
            ExtraAccountMeta::new_with_pubkey(&ctx.accounts.nft_deposit.key(), false, true)?,
            ExtraAccountMeta::new_with_pubkey(&ctx.accounts.raydium_pool_ata.key(), false, true)?,
        ];
    
        ExtraAccountMetaList::init::<ExecuteInstruction>(
            &mut ctx.accounts.extra_account_meta_list.try_borrow_mut_data()?,
            &extra_account_metas,
        )?;
    
        Ok(())
    }

}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct TransferHook<'info> {
    /// CHECK: Source token account
    pub source: AccountInfo<'info>,
    /// CHECK: Mint (LIQUID_MINT)
    pub mint: AccountInfo<'info>,
    /// CHECK: Destination token account
    pub destination: AccountInfo<'info>,
    /// CHECK: Authority
    pub authority: AccountInfo<'info>,
    /// CHECK: Extra account PDA
    #[account(seeds = [b"extra-account-metas", mint.key().as_ref()], bump)]
    pub extra_account: AccountInfo<'info>,
    #[account(mut)]
    pub state: Account<'info, HookState>,
    /// CHECK: NFT deposit
    #[account(mut)]
    pub nft_deposit: AccountInfo<'info>,
    /// CHECK: Token deposit
    #[account(mut)]
    pub token_deposit: AccountInfo<'info>,
    /// CHECK: Raydium LP ATA
    #[account(mut)]
    pub raydium_pool_ata: AccountInfo<'info>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub payer: Signer<'info>,
}

#[derive(Accounts)]
pub struct InitializeExtraAccountMetaList<'info> {
    #[account(mut)]
    payer: Signer<'info>,

    /// CHECK: ExtraAccountMetaList Account, must use these seeds
    #[account(
        init,
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump,
        space = ExtraAccountMetaList::size_of(
            InitializeExtraAccountMetaList::extra_account_metas()?.len()
        )?,
        payer = payer
    )]
    pub extra_account_meta_list: AccountInfo<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub system_program: Program<'info, System>,
    #[account(
        init_if_needed,
        seeds = [b"state"],
        bump,
        payer = payer,
        space = 8 + 32 + 32 + 32 + 32 + 100 * 32 + 100 * 8 // Discriminator + authority + deposits + ownership + available_nfts
    )]
    pub state: Account<'info, HookState>,
    #[account(mut)]
    pub token_deposit: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub nft_deposit: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub raydium_pool_ata: InterfaceAccount<'info, TokenAccount>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

// Define extra account metas to store on extra_account_meta_list account
impl<'info> InitializeExtraAccountMetaList<'info> {
    pub fn extra_account_metas() -> Result<Vec<ExtraAccountMeta>> {
        Ok(
            vec![
                ExtraAccountMeta::new_with_seeds(
                    &[
                        Seed::Literal {
                            bytes: b"counter".to_vec(),
                        },
                    ],
                    false, // is_signer
                    true // is_writable
                )?
            ]
        )
    }
}


#[account]
pub struct HookState {
    pub authority: Pubkey,
    pub nft_availability: Box<Vec<bool>>, // Boxed Vec on heap
    pub nft_deposit_atas: Box<Vec<Pubkey>>, // Boxed Vec on heap
}

#[derive(Clone, Copy, AnchorSerialize, AnchorDeserialize)]
pub struct NftAvailability {
    pub ata_pubkey: Pubkey, // The ATA’s pubkey
    pub available: bool,      // Current owner (starts as authority)
}