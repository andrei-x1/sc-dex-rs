elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use common_structs::FftTokenAmountPair;

#[derive(TopEncode)]
pub struct SwapEvent<M: ManagedTypeApi> {
    caller: Address,
    token_amount_in: FftTokenAmountPair<M>,
    token_amount_out: FftTokenAmountPair<M>,
    fee_amount: BigUint<M>,
    pair_reserves: Vec<FftTokenAmountPair<M>>,
    block: u64,
    epoch: u64,
    timestamp: u64,
}

#[derive(TopEncode)]
pub struct SwapNoFeeAndForwardEvent<M: ManagedTypeApi> {
    caller: Address,
    swap_out_token_amount: FftTokenAmountPair<M>,
    destination: Address,
    block: u64,
    epoch: u64,
    timestamp: u64,
}

#[derive(TopEncode)]
pub struct AddLiquidityEvent<M: ManagedTypeApi> {
    caller: Address,
    first_token_amount: FftTokenAmountPair<M>,
    second_token_amount: FftTokenAmountPair<M>,
    lp_token_amount: FftTokenAmountPair<M>,
    lp_supply: BigUint<M>,
    pair_reserves: Vec<FftTokenAmountPair<M>>,
    block: u64,
    epoch: u64,
    timestamp: u64,
}

#[derive(TopEncode)]
pub struct RemoveLiquidityEvent<M: ManagedTypeApi> {
    caller: Address,
    first_token_amount: FftTokenAmountPair<M>,
    second_token_amount: FftTokenAmountPair<M>,
    lp_token_amount: FftTokenAmountPair<M>,
    lp_supply: BigUint<M>,
    pair_reserves: Vec<FftTokenAmountPair<M>>,
    block: u64,
    epoch: u64,
    timestamp: u64,
}

#[elrond_wasm::module]
pub trait EventsModule {
    fn emit_swap_event(
        &self,
        caller: Address,
        token_amount_in: FftTokenAmountPair<Self::TypeManager>,
        token_amount_out: FftTokenAmountPair<Self::TypeManager>,
        fee_amount: BigUint,
        pair_reserves: Vec<FftTokenAmountPair<Self::TypeManager>>,
    ) {
        let epoch = self.blockchain().get_block_epoch();
        self.swap_event(
            token_amount_in.token_id.clone(),
            token_amount_out.token_id.clone(),
            caller.clone(),
            epoch,
            SwapEvent {
                caller,
                token_amount_in,
                token_amount_out,
                fee_amount,
                pair_reserves,
                block: self.blockchain().get_block_nonce(),
                epoch,
                timestamp: self.blockchain().get_block_timestamp(),
            },
        )
    }

    fn emit_swap_no_fee_and_forward_event(
        &self,
        caller: Address,
        swap_out_token_amount: FftTokenAmountPair<Self::TypeManager>,
        destination: Address,
    ) {
        let epoch = self.blockchain().get_block_epoch();
        self.swap_no_fee_and_forward_event(
            swap_out_token_amount.token_id.clone(),
            caller.clone(),
            epoch,
            SwapNoFeeAndForwardEvent {
                caller,
                swap_out_token_amount,
                destination,
                block: self.blockchain().get_block_nonce(),
                epoch,
                timestamp: self.blockchain().get_block_timestamp(),
            },
        )
    }

    fn emit_add_liquidity_event(
        &self,
        caller: Address,
        first_token_amount: FftTokenAmountPair<Self::TypeManager>,
        second_token_amount: FftTokenAmountPair<Self::TypeManager>,
        lp_token_amount: FftTokenAmountPair<Self::TypeManager>,
        lp_supply: BigUint,
        pair_reserves: Vec<FftTokenAmountPair<Self::TypeManager>>,
    ) {
        let epoch = self.blockchain().get_block_epoch();
        self.add_liquidity_event(
            first_token_amount.token_id.clone(),
            second_token_amount.token_id.clone(),
            caller.clone(),
            epoch,
            AddLiquidityEvent {
                caller,
                first_token_amount,
                second_token_amount,
                lp_token_amount,
                lp_supply,
                pair_reserves,
                block: self.blockchain().get_block_nonce(),
                epoch,
                timestamp: self.blockchain().get_block_timestamp(),
            },
        )
    }

    fn emit_remove_liquidity_event(
        &self,
        caller: Address,
        first_token_amount: FftTokenAmountPair<Self::TypeManager>,
        second_token_amount: FftTokenAmountPair<Self::TypeManager>,
        lp_token_amount: FftTokenAmountPair<Self::TypeManager>,
        lp_supply: BigUint,
        pair_reserves: Vec<FftTokenAmountPair<Self::TypeManager>>,
    ) {
        let epoch = self.blockchain().get_block_epoch();
        self.remove_liquidity_event(
            first_token_amount.token_id.clone(),
            second_token_amount.token_id.clone(),
            caller.clone(),
            epoch,
            RemoveLiquidityEvent {
                caller,
                first_token_amount,
                second_token_amount,
                lp_token_amount,
                lp_supply,
                pair_reserves,
                block: self.blockchain().get_block_nonce(),
                epoch,
                timestamp: self.blockchain().get_block_timestamp(),
            },
        )
    }

    #[event("swap")]
    fn swap_event(
        &self,
        #[indexed] token_in: TokenIdentifier,
        #[indexed] token_out: TokenIdentifier,
        #[indexed] caller: Address,
        #[indexed] epoch: u64,
        swap_event: SwapEvent<Self::TypeManager>,
    );

    #[event("swap_no_fee_and_forward")]
    fn swap_no_fee_and_forward_event(
        &self,
        #[indexed] swap_out_token: TokenIdentifier,
        #[indexed] caller: Address,
        #[indexed] epoch: u64,
        swap_no_fee_and_forward_event: SwapNoFeeAndForwardEvent<Self::TypeManager>,
    );

    #[event("add_liquidity")]
    fn add_liquidity_event(
        &self,
        #[indexed] first_token: TokenIdentifier,
        #[indexed] second_token: TokenIdentifier,
        #[indexed] caller: Address,
        #[indexed] epoch: u64,
        add_liquidity_event: AddLiquidityEvent<Self::TypeManager>,
    );

    #[event("remove_liquidity")]
    fn remove_liquidity_event(
        &self,
        #[indexed] first_token: TokenIdentifier,
        #[indexed] second_token: TokenIdentifier,
        #[indexed] caller: Address,
        #[indexed] epoch: u64,
        remove_liquidity_event: RemoveLiquidityEvent<Self::TypeManager>,
    );
}
