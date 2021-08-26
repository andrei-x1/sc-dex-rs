use common_structs::{FftTokenAmountPair, GenericTokenAmountPair, WrappedLpTokenAttributes};

use super::proxy_common;
use proxy_common::ACCEPT_PAY_FUNC_NAME;

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use super::proxy_pair;
use nft_deposit::ProxyTrait as _;
use proxy_pair::WrappedLpToken;
use sc_locked_asset_factory::locked_asset_token_merge::ProxyTrait as _;

#[elrond_wasm::module]
pub trait WrappedLpTokenMerge:
    token_merge::TokenMergeModule
    + token_send::TokenSendModule
    + token_supply::TokenSupplyModule
    + proxy_common::ProxyCommonModule
    + nft_deposit::NftDepositModule
{
    #[proxy]
    fn locked_asset_factory(&self, to: Address) -> sc_locked_asset_factory::Proxy<Self::SendApi>;

    #[endpoint(mergeWrappedLpTokens)]
    fn merge_wrapped_lp_tokens(
        &self,
        #[var_args] opt_accept_funds_func: OptionalArg<BoxedBytes>,
    ) -> SCResult<()> {
        let caller = self.blockchain().get_caller();
        self.merge_wrapped_lp_tokens_and_send(&caller, Option::None, opt_accept_funds_func)?;
        Ok(())
    }

    fn merge_wrapped_lp_tokens_and_send(
        &self,
        caller: &Address,
        replic: Option<WrappedLpToken<Self::TypeManager>>,
        opt_accept_funds_func: OptionalArg<BoxedBytes>,
    ) -> SCResult<(WrappedLpToken<Self::TypeManager>, bool)> {
        let deposit = self.nft_deposit(caller).get();
        require!(!deposit.is_empty() || replic.is_some(), "Empty deposit");
        let deposit_len = deposit.len();

        let wrapped_lp_token_id = self.wrapped_lp_token_id().get();
        self.require_all_tokens_are_wrapped_lp_tokens(&deposit, &wrapped_lp_token_id)?;

        let mut tokens = self.get_wrapped_lp_tokens_from_deposit(&deposit)?;

        if replic.is_some() {
            tokens.push(replic.unwrap());
        }
        self.require_wrapped_lp_tokens_from_same_pair(&tokens)?;

        let merged_locked_token_amount = self.merge_locked_asset_tokens_from_wrapped_lp(&tokens)?;
        let merged_wrapped_lp_amount = self.get_merged_wrapped_lp_tokens_amount(&tokens);
        let lp_token_amount = FftTokenAmountPair {
            token_id: tokens[0].attributes.lp_token_id.clone(),
            amount: merged_wrapped_lp_amount.clone(),
        };

        let attrs = self
            .get_merged_wrapped_lp_token_attributes(&lp_token_amount, &merged_locked_token_amount);
        self.burn_deposit_tokens(caller, &deposit);

        self.nft_create_tokens(&wrapped_lp_token_id, &merged_wrapped_lp_amount, &attrs);
        let new_nonce = self.increase_wrapped_lp_token_nonce();

        self.send_nft_tokens(
            &wrapped_lp_token_id,
            new_nonce,
            &merged_wrapped_lp_amount,
            caller,
            &opt_accept_funds_func,
        )?;

        let new_token = WrappedLpToken {
            token_amount: GenericTokenAmountPair {
                token_id: wrapped_lp_token_id,
                token_nonce: new_nonce,
                amount: merged_wrapped_lp_amount,
            },
            attributes: attrs,
        };
        let is_merged = deposit_len != 0;

        Ok((new_token, is_merged))
    }

    fn require_deposit_empty_or_tokens_are_wrapped_lp_tokens(&self) -> SCResult<()> {
        let wrapped_farm_token_id = self.wrapped_lp_token_id().get();
        let caller = self.blockchain().get_caller();
        let deposit = self.nft_deposit(&caller).get();
        self.require_all_tokens_are_wrapped_lp_tokens(&deposit, &wrapped_farm_token_id)
    }

    fn get_wrapped_lp_tokens_from_deposit(
        &self,
        deposit: &[GenericTokenAmountPair<Self::TypeManager>],
    ) -> SCResult<Vec<WrappedLpToken<Self::TypeManager>>> {
        let mut result = Vec::new();

        for elem in deposit.iter() {
            result.push(WrappedLpToken {
                token_amount: elem.clone(),
                attributes: self
                    .get_wrapped_lp_token_attributes(&elem.token_id, elem.token_nonce)?,
            })
        }
        Ok(result)
    }

    fn require_wrapped_lp_tokens_from_same_pair(
        &self,
        tokens: &[WrappedLpToken<Self::TypeManager>],
    ) -> SCResult<()> {
        let lp_token_id = tokens[0].attributes.lp_token_id.clone();

        for elem in tokens.iter() {
            require!(
                elem.attributes.lp_token_id == lp_token_id,
                "Lp token id differs"
            );
        }
        Ok(())
    }

    fn require_all_tokens_are_wrapped_lp_tokens(
        &self,
        tokens: &[GenericTokenAmountPair<Self::TypeManager>],
        wrapped_lp_token_id: &TokenIdentifier,
    ) -> SCResult<()> {
        for elem in tokens.iter() {
            require!(
                &elem.token_id == wrapped_lp_token_id,
                "Not a Wrapped Lp Token"
            );
        }
        Ok(())
    }

    fn get_merged_wrapped_lp_token_attributes(
        &self,
        lp_token_amount: &FftTokenAmountPair<Self::TypeManager>,
        merged_locked_asset_token_amount: &GenericTokenAmountPair<Self::TypeManager>,
    ) -> WrappedLpTokenAttributes<Self::TypeManager> {
        WrappedLpTokenAttributes {
            lp_token_id: lp_token_amount.token_id.clone(),
            lp_token_total_amount: lp_token_amount.amount.clone(),
            locked_assets_invested: merged_locked_asset_token_amount.amount.clone(),
            locked_assets_nonce: merged_locked_asset_token_amount.token_nonce,
        }
    }

    fn merge_locked_asset_tokens_from_wrapped_lp(
        &self,
        tokens: &[WrappedLpToken<Self::TypeManager>],
    ) -> SCResult<GenericTokenAmountPair<Self::TypeManager>> {
        let locked_asset_factory_addr = self.locked_asset_factory_address().get();
        let locked_asset_token = self.locked_asset_token_id().get();

        if tokens.len() == 1 {
            let token = tokens[0].clone();

            let amount = self.rule_of_three_non_zero_result(
                &token.token_amount.amount,
                &token.attributes.lp_token_total_amount,
                &token.attributes.locked_assets_invested,
            )?;
            return Ok(GenericTokenAmountPair {
                token_id: locked_asset_token,
                token_nonce: token.attributes.locked_assets_nonce,
                amount,
            });
        }

        for entry in tokens.iter() {
            let amount = self.rule_of_three_non_zero_result(
                &entry.token_amount.amount,
                &entry.attributes.lp_token_total_amount,
                &entry.attributes.locked_assets_invested,
            )?;

            self.locked_asset_factory(locked_asset_factory_addr.clone())
                .deposit_tokens(
                    locked_asset_token.clone(),
                    entry.attributes.locked_assets_nonce,
                    amount,
                )
                .execute_on_dest_context();
        }

        Ok(self
            .locked_asset_factory(locked_asset_factory_addr)
            .merge_locked_asset_tokens(OptionalArg::Some(BoxedBytes::from(ACCEPT_PAY_FUNC_NAME)))
            .execute_on_dest_context_custom_range(|_, after| (after - 1, after)))
    }

    fn get_merged_wrapped_lp_tokens_amount(
        &self,
        tokens: &[WrappedLpToken<Self::TypeManager>],
    ) -> BigUint {
        let mut token_amount = self.types().big_uint_zero();

        tokens
            .iter()
            .for_each(|x| token_amount += &x.token_amount.amount);
        token_amount
    }
}
