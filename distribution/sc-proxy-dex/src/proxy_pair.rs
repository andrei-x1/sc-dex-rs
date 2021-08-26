#![allow(clippy::too_many_arguments)]
#![allow(clippy::comparison_chain)]

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use core::iter::FromIterator;

use proxy_common::ACCEPT_PAY_FUNC_NAME;

use common_structs::{FftTokenAmountPair, GenericTokenAmountPair, Nonce, WrappedLpTokenAttributes};
use elrond_dex_pair::config::ProxyTrait as _;

use super::events;
use super::proxy_common;
use super::wrapped_lp_token_merge;

type AddLiquidityResultType<ManagedTypeApi> = MultiResult3<
    FftTokenAmountPair<ManagedTypeApi>,
    FftTokenAmountPair<ManagedTypeApi>,
    FftTokenAmountPair<ManagedTypeApi>,
>;

type RemoveLiquidityResultType<ManagedTypeApi> =
    MultiResult2<FftTokenAmountPair<ManagedTypeApi>, FftTokenAmountPair<ManagedTypeApi>>;

#[derive(Clone)]
pub struct WrappedLpToken<M: ManagedTypeApi> {
    pub token_amount: GenericTokenAmountPair<M>,
    pub attributes: WrappedLpTokenAttributes<M>,
}

#[elrond_wasm::module]
pub trait ProxyPairModule:
    proxy_common::ProxyCommonModule
    + token_supply::TokenSupplyModule
    + wrapped_lp_token_merge::WrappedLpTokenMerge
    + token_merge::TokenMergeModule
    + token_send::TokenSendModule
    + nft_deposit::NftDepositModule
    + events::EventsModule
{
    #[proxy]
    fn pair_contract_proxy(&self, to: Address) -> elrond_dex_pair::Proxy<Self::SendApi>;

    #[endpoint(addPairToIntermediate)]
    fn add_pair_to_intermediate(&self, pair_address: Address) -> SCResult<()> {
        self.require_permissions()?;
        self.intermediated_pairs().insert(pair_address);
        Ok(())
    }

    #[endpoint(removeIntermediatedPair)]
    fn remove_intermediated_pair(&self, pair_address: Address) -> SCResult<()> {
        self.require_permissions()?;
        self.require_is_intermediated_pair(&pair_address)?;
        self.intermediated_pairs().remove(&pair_address);
        Ok(())
    }

    #[payable("*")]
    #[endpoint(addLiquidityProxy)]
    fn add_liquidity_proxy(
        &self,
        pair_address: Address,
        first_token_amount_min: BigUint,
        second_token_amount_min: BigUint,
    ) -> SCResult<()> {
        self.require_is_intermediated_pair(&pair_address)?;
        self.require_wrapped_lp_token_id_not_empty()?;
        self.require_deposit_empty_or_tokens_are_wrapped_lp_tokens()?;
        let payments = self.call_value().get_all_esdt_transfers();
        require!(payments.len() == 2, "Panyments len should be 2");

        let first_token_id = payments[0].token_name.clone();
        let first_token_nonce = payments[0].token_nonce;
        let first_token_amount_desired = payments[0].amount.clone();
        let second_token_id = payments[1].token_name.clone();
        let second_token_nonce = payments[1].token_nonce;
        let second_token_amount_desired = payments[1].amount.clone();

        let caller = self.blockchain().get_caller();
        require!(first_token_id != second_token_id, "Identical tokens");
        require!(
            (first_token_nonce == 0 && second_token_nonce != 0)
                || (first_token_nonce != 0 && second_token_nonce == 0),
            "This endpoint accepts one Fungible and one SemiFungible"
        );
        require!(
            first_token_amount_desired > 0 && second_token_amount_desired > 0,
            "Cannot add zero amount"
        );
        let locked_asset_token_id = self.locked_asset_token_id().get();
        require!(
            (first_token_nonce != 0 && first_token_id == locked_asset_token_id)
                || (second_token_nonce != 0 && second_token_id == locked_asset_token_id),
            "The SemiFungible token should be the locked asset"
        );

        // Actual adding of liquidity
        let result = self.actual_add_liquidity(
            &pair_address,
            &first_token_id,
            first_token_nonce,
            &first_token_amount_desired,
            &first_token_amount_min,
            &second_token_id,
            second_token_nonce,
            &second_token_amount_desired,
            &second_token_amount_min,
        );

        let result_tuple = result.0;
        let lp_received = result_tuple.0;
        let first_token_used = result_tuple.1;
        let second_token_used = result_tuple.2;
        require!(
            lp_received.amount > 0,
            "LP token amount should be greater than 0"
        );
        require!(
            first_token_used.token_id == first_token_id
                || second_token_used.token_id == second_token_id,
            "Bad token order"
        );
        require!(
            first_token_used.amount <= first_token_amount_desired
                && second_token_used.amount <= second_token_amount_desired,
            "Used more tokens than provided"
        );

        //Recalculate temporary funds and burn unused
        let locked_asset_token_nonce: Nonce;
        let consumed_locked_tokens: BigUint;
        let asset_token_id = self.asset_token_id().get();
        let unused_minted_assets: BigUint;
        if first_token_used.token_id == asset_token_id {
            consumed_locked_tokens = first_token_used.amount.clone();
            unused_minted_assets = &first_token_amount_desired - &consumed_locked_tokens;
            locked_asset_token_nonce = first_token_nonce;
        } else if second_token_used.token_id == asset_token_id {
            consumed_locked_tokens = second_token_used.amount.clone();
            unused_minted_assets = &second_token_amount_desired - &consumed_locked_tokens;
            locked_asset_token_nonce = second_token_nonce;
        } else {
            return sc_error!("Add liquidity did not return asset token id");
        }

        let (new_wrapped_lp_token, created_with_merge) = self.create_by_merging_and_send(
            &lp_received.token_id,
            &lp_received.amount,
            &consumed_locked_tokens,
            locked_asset_token_nonce,
            &caller,
        )?;

        let unused_first_token_payment = EsdtTokenPayment::from(
            first_token_id.clone(),
            first_token_nonce,
            &first_token_amount_desired - &first_token_used.amount,
        );
        let unused_second_token_payment = EsdtTokenPayment::from(
            second_token_id.clone(),
            second_token_nonce,
            &second_token_amount_desired - &second_token_used.amount,
        );
        self.send_multiple_tokens_compact(
            &[unused_first_token_payment, unused_second_token_payment],
            &caller,
            &OptionalArg::None,
        )?;

        if unused_minted_assets > 0 {
            self.burn_tokens(&asset_token_id, &unused_minted_assets);
        }

        let first_token_amount = GenericTokenAmountPair {
            token_id: first_token_id,
            token_nonce: first_token_nonce,
            amount: first_token_used.amount,
        };
        let second_token_amount = GenericTokenAmountPair {
            token_id: second_token_id,
            token_nonce: second_token_nonce,
            amount: second_token_used.amount,
        };
        self.emit_add_liquidity_proxy_event(
            caller,
            pair_address,
            first_token_amount,
            second_token_amount,
            new_wrapped_lp_token.token_amount,
            new_wrapped_lp_token.attributes,
            created_with_merge,
        );
        Ok(())
    }

    #[payable("*")]
    #[endpoint(removeLiquidityProxy)]
    fn remove_liquidity_proxy(
        &self,
        #[payment_token] token_id: TokenIdentifier,
        #[payment_amount] amount: BigUint,
        #[payment_nonce] token_nonce: Nonce,
        pair_address: Address,
        first_token_amount_min: BigUint,
        second_token_amount_min: BigUint,
    ) -> SCResult<()> {
        self.require_is_intermediated_pair(&pair_address)?;
        self.require_wrapped_lp_token_id_not_empty()?;
        require!(token_nonce != 0, "Can only be called with an SFT");
        require!(amount != 0, "Payment amount cannot be zero");

        let wrapped_lp_token_id = self.wrapped_lp_token_id().get();
        require!(token_id == wrapped_lp_token_id, "Wrong input token");

        let caller = self.blockchain().get_caller();
        let lp_token_id = self.ask_for_lp_token_id(&pair_address);
        let attributes = self.get_wrapped_lp_token_attributes(&token_id, token_nonce)?;
        require!(lp_token_id == attributes.lp_token_id, "Bad input address");

        let locked_asset_token_id = self.locked_asset_token_id().get();
        let asset_token_id = self.asset_token_id().get();

        let tokens_for_position = self
            .actual_remove_liquidity(
                &pair_address,
                &lp_token_id,
                &amount,
                &first_token_amount_min,
                &second_token_amount_min,
            )
            .into_tuple();

        let fungible_token_id: TokenIdentifier;
        let fungible_token_amount: BigUint;
        let assets_received: BigUint;
        let locked_assets_invested = self.rule_of_three_non_zero_result(
            &amount,
            &attributes.lp_token_total_amount,
            &attributes.locked_assets_invested,
        )?;

        if tokens_for_position.0.token_id == asset_token_id {
            assets_received = tokens_for_position.0.amount.clone();
            fungible_token_id = tokens_for_position.1.token_id.clone();
            fungible_token_amount = tokens_for_position.1.amount.clone();
        } else if tokens_for_position.1.token_id == asset_token_id {
            assets_received = tokens_for_position.1.amount.clone();
            fungible_token_id = tokens_for_position.0.token_id.clone();
            fungible_token_amount = tokens_for_position.0.amount.clone();
        } else {
            return sc_error!("Bad tokens received from pair SC");
        }

        //Send back the tokens removed from pair sc.
        self.send()
            .direct(&caller, &fungible_token_id, 0, &fungible_token_amount, &[]);
        let locked_assets_to_send =
            core::cmp::min(assets_received.clone(), locked_assets_invested.clone());
        self.send().direct(
            &caller,
            &locked_asset_token_id,
            attributes.locked_assets_nonce,
            &locked_assets_to_send,
            &[],
        );

        //Do cleanup
        if assets_received > locked_assets_invested {
            let difference = assets_received - locked_assets_invested;
            self.send()
                .direct(&caller, &asset_token_id, 0, &difference, &[]);
        } else if assets_received < locked_assets_invested {
            let difference = locked_assets_invested - assets_received;
            self.nft_burn_tokens(
                &locked_asset_token_id,
                attributes.locked_assets_nonce,
                &difference,
            );
        }

        self.burn_tokens(&asset_token_id, &locked_assets_to_send);
        self.nft_burn_tokens(&wrapped_lp_token_id, token_nonce, &amount);

        let wrapped_lp_token_amount = GenericTokenAmountPair {
            token_id,
            token_nonce,
            amount,
        };
        let first_token_amount = GenericTokenAmountPair {
            token_id: tokens_for_position.0.token_id,
            token_nonce: 0,
            amount: tokens_for_position.0.amount,
        };
        let second_token_amount = GenericTokenAmountPair {
            token_id: tokens_for_position.1.token_id,
            token_nonce: 0,
            amount: tokens_for_position.1.amount,
        };
        self.emit_remove_liquidity_proxy_event(
            caller,
            pair_address,
            wrapped_lp_token_amount,
            attributes,
            first_token_amount,
            second_token_amount,
        );
        Ok(())
    }

    fn actual_add_liquidity(
        &self,
        pair_address: &Address,
        first_token_id: &TokenIdentifier,
        first_token_nonce: u64,
        first_token_amount_desired: &BigUint,
        first_token_amount_min: &BigUint,
        second_token_id: &TokenIdentifier,
        _second_token_nonce: u64,
        second_token_amount_desired: &BigUint,
        second_token_amount_min: &BigUint,
    ) -> AddLiquidityResultType<Self::TypeManager> {
        let asset_token_id = self.asset_token_id().get();
        let mut payments = Vec::new();
        let first_token_to_send: &TokenIdentifier;
        let second_token_to_send: &TokenIdentifier;

        if first_token_nonce == 0 {
            first_token_to_send = first_token_id;
            self.mint_tokens(&asset_token_id, second_token_amount_desired);
            second_token_to_send = &asset_token_id;
        } else {
            second_token_to_send = second_token_id;
            self.mint_tokens(&asset_token_id, first_token_amount_desired);
            first_token_to_send = &asset_token_id;
        };

        payments.push(EsdtTokenPayment::from(
            first_token_to_send.clone(),
            0,
            first_token_amount_desired.clone(),
        ));
        payments.push(EsdtTokenPayment::from(
            second_token_to_send.clone(),
            0,
            second_token_amount_desired.clone(),
        ));

        self.pair_contract_proxy(pair_address.clone())
            .add_liquidity(
                first_token_amount_min.clone(),
                second_token_amount_min.clone(),
                OptionalArg::Some(BoxedBytes::from(ACCEPT_PAY_FUNC_NAME)),
            )
            .with_multi_token_transfer(payments)
            .execute_on_dest_context()
    }

    fn actual_remove_liquidity(
        &self,
        pair_address: &Address,
        lp_token_id: &TokenIdentifier,
        liquidity: &BigUint,
        first_token_amount_min: &BigUint,
        second_token_amount_min: &BigUint,
    ) -> RemoveLiquidityResultType<Self::TypeManager> {
        self.pair_contract_proxy(pair_address.clone())
            .remove_liquidity(
                lp_token_id.clone(),
                liquidity.clone(),
                first_token_amount_min.clone(),
                second_token_amount_min.clone(),
                OptionalArg::Some(BoxedBytes::from(ACCEPT_PAY_FUNC_NAME)),
            )
            .execute_on_dest_context()
    }

    fn ask_for_lp_token_id(&self, pair_address: &Address) -> TokenIdentifier {
        self.pair_contract_proxy(pair_address.clone())
            .get_lp_token_identifier()
            .execute_on_dest_context()
    }

    fn create_by_merging_and_send(
        &self,
        lp_token_id: &TokenIdentifier,
        lp_token_amount: &BigUint,
        locked_tokens_consumed: &BigUint,
        locked_tokens_nonce: Nonce,
        caller: &Address,
    ) -> SCResult<(WrappedLpToken<Self::TypeManager>, bool)> {
        self.merge_wrapped_lp_tokens_and_send(
            caller,
            Option::Some(WrappedLpToken {
                token_amount: GenericTokenAmountPair {
                    token_id: self.wrapped_lp_token_id().get(),
                    token_nonce: 0,
                    amount: lp_token_amount.clone(),
                },
                attributes: WrappedLpTokenAttributes {
                    lp_token_id: lp_token_id.clone(),
                    lp_token_total_amount: lp_token_amount.clone(),
                    locked_assets_invested: locked_tokens_consumed.clone(),
                    locked_assets_nonce: locked_tokens_nonce,
                },
            }),
            OptionalArg::None,
        )
    }

    fn require_is_intermediated_pair(&self, address: &Address) -> SCResult<()> {
        require!(
            self.intermediated_pairs().contains(address),
            "Not an intermediated pair"
        );
        Ok(())
    }

    fn require_wrapped_lp_token_id_not_empty(&self) -> SCResult<()> {
        require!(!self.wrapped_lp_token_id().is_empty(), "Empty token id");
        Ok(())
    }

    #[view(getTemporaryFunds)]
    fn get_temporary_funds(
        &self,
        address: &Address,
    ) -> MultiResultVec<GenericTokenAmountPair<Self::TypeManager>> {
        MultiResultVec::from_iter(
            self.temporary_funds(address)
                .iter()
                .map(|x| {
                    let (key, amount) = x;
                    let (token_id, token_nonce) = key;
                    GenericTokenAmountPair {
                        token_id,
                        token_nonce,
                        amount,
                    }
                })
                .collect::<Vec<GenericTokenAmountPair<Self::TypeManager>>>(),
        )
    }

    #[storage_mapper("funds")]
    fn temporary_funds(
        &self,
        user: &Address,
    ) -> SafeMapMapper<Self::Storage, (TokenIdentifier, Nonce), BigUint>;
}
