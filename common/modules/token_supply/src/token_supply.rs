#![no_std]

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use core::iter::FromIterator;

use common_structs::Nonce;

#[elrond_wasm::module]
pub trait TokenSupplyModule {
    fn nft_create_tokens<T: elrond_codec::TopEncode>(
        &self,
        token_id: &TokenIdentifier,
        amount: &Self::BigUint,
        attributes: &T,
    ) {
        self.increase_generated_amount(token_id, amount);
        self.send().esdt_nft_create::<T>(
            token_id,
            amount,
            &BoxedBytes::empty(),
            &0u64.into(),
            &BoxedBytes::empty(),
            attributes,
            &[BoxedBytes::empty()],
        );
    }

    fn nft_add_quantity_tokens(
        &self,
        token_id: &TokenIdentifier,
        nonce: Nonce,
        amount: &Self::BigUint,
    ) {
        self.increase_generated_amount(token_id, amount);
        self.send().esdt_local_mint(token_id, nonce, amount);
    }

    fn nft_burn_tokens(&self, token_id: &TokenIdentifier, nonce: Nonce, amount: &Self::BigUint) {
        self.increase_burned_amount(token_id, amount);
        self.send().esdt_local_burn(token_id, nonce, amount);
    }

    fn mint_tokens(&self, token_id: &TokenIdentifier, amount: &Self::BigUint) {
        self.increase_generated_amount(token_id, amount);
        self.send().esdt_local_mint(token_id, 0, amount);
    }

    fn burn_tokens(&self, token_id: &TokenIdentifier, amount: &Self::BigUint) {
        self.increase_burned_amount(token_id, amount);
        self.send().esdt_local_burn(token_id, 0, amount);
    }

    fn increase_generated_amount(&self, token_id: &TokenIdentifier, amount: &Self::BigUint) {
        let old_amount = self.get_generated_token_amount(token_id);
        self.generated_tokens()
            .insert(token_id.clone(), &old_amount + amount);
    }

    fn increase_burned_amount(&self, token_id: &TokenIdentifier, amount: &Self::BigUint) {
        let old_amount = self.get_burned_token_amount(token_id);
        self.burned_tokens()
            .insert(token_id.clone(), &old_amount + amount);
    }

    fn get_total_supply(&self, token_id: &TokenIdentifier) -> SCResult<Self::BigUint> {
        let generated_amount = self.get_generated_token_amount(token_id);
        let burned_amount = self.get_burned_token_amount(token_id);
        require!(generated_amount >= burned_amount, "Negative total supply");
        Ok(generated_amount - burned_amount)
    }

    #[view(getGeneratedTokenAmountList)]
    fn get_genereated_token_amount_list(&self) -> MultiResultVec<(TokenIdentifier, Self::BigUint)> {
        MultiResultVec::from_iter(
            self.generated_tokens()
                .iter()
                .collect::<Vec<(TokenIdentifier, Self::BigUint)>>(),
        )
    }

    #[view(getBurnedTokenAmountList)]
    fn get_burned_token_amount_list(&self) -> MultiResultVec<(TokenIdentifier, Self::BigUint)> {
        MultiResultVec::from_iter(
            self.burned_tokens()
                .iter()
                .collect::<Vec<(TokenIdentifier, Self::BigUint)>>(),
        )
    }

    #[view(getGeneratedTokenAmount)]
    fn get_generated_token_amount(&self, token_id: &TokenIdentifier) -> Self::BigUint {
        self.generated_tokens().get(token_id).unwrap_or_default()
    }

    #[view(getBurnedTokenAmount)]
    fn get_burned_token_amount(&self, token_id: &TokenIdentifier) -> Self::BigUint {
        self.burned_tokens().get(token_id).unwrap_or_default()
    }

    #[storage_mapper("generated_tokens")]
    fn generated_tokens(&self) -> SafeMapMapper<Self::Storage, TokenIdentifier, Self::BigUint>;

    #[storage_mapper("burned_tokens")]
    fn burned_tokens(&self) -> SafeMapMapper<Self::Storage, TokenIdentifier, Self::BigUint>;
}
