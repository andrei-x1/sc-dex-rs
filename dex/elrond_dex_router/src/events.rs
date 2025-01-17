elrond_wasm::imports!();
elrond_wasm::derive_imports!();

#[derive(TopEncode)]
pub struct CreatePairEvent {
    caller: Address,
    first_token_id: TokenIdentifier,
    second_token_id: TokenIdentifier,
    total_fee_percent: u64,
    special_fee_percent: u64,
    pair_address: Address,
    block: u64,
    epoch: u64,
    timestamp: u64,
}

#[elrond_wasm::module]
pub trait EventsModule {
    fn emit_create_pair_event(
        self,
        caller: Address,
        first_token_id: TokenIdentifier,
        second_token_id: TokenIdentifier,
        total_fee_percent: u64,
        special_fee_percent: u64,
        pair_address: Address,
    ) {
        let epoch = self.blockchain().get_block_epoch();
        self.create_pair_event(
            first_token_id.clone(),
            second_token_id.clone(),
            caller.clone(),
            epoch,
            CreatePairEvent {
                caller,
                first_token_id,
                second_token_id,
                total_fee_percent,
                special_fee_percent,
                pair_address,
                block: self.blockchain().get_block_nonce(),
                epoch,
                timestamp: self.blockchain().get_block_timestamp(),
            },
        )
    }

    #[event("create_pair")]
    fn create_pair_event(
        self,
        #[indexed] first_token_id: TokenIdentifier,
        #[indexed] second_token_id: TokenIdentifier,
        #[indexed] caller: Address,
        #[indexed] epoch: u64,
        swap_event: CreatePairEvent,
    );
}
