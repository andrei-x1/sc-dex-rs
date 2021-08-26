elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use common_structs::{FftTokenAmountPair, GenericTokenAmountPair};

use crate::FarmTokenAttributes;

#[derive(TopEncode)]
pub struct EnterFarmEvent<M: ManagedTypeApi> {
    caller: Address,
    farming_token_amount: FftTokenAmountPair<M>,
    farming_reserve: BigUint<M>,
    farm_token_amount: GenericTokenAmountPair<M>,
    farm_supply: BigUint<M>,
    reward_token_reserve: FftTokenAmountPair<M>,
    farm_attributes: FarmTokenAttributes<M>,
    created_with_merge: bool,
    block: u64,
    epoch: u64,
    timestamp: u64,
}

#[derive(TopEncode)]
pub struct ExitFarmEvent<M: ManagedTypeApi> {
    caller: Address,
    farming_token_amount: FftTokenAmountPair<M>,
    farming_reserve: BigUint<M>,
    farm_token_amount: GenericTokenAmountPair<M>,
    farm_supply: BigUint<M>,
    reward_token_amount: GenericTokenAmountPair<M>,
    reward_reserve: BigUint<M>,
    farm_attributes: FarmTokenAttributes<M>,
    block: u64,
    epoch: u64,
    timestamp: u64,
}

#[derive(TopEncode)]
pub struct ClaimRewardsEvent<M: ManagedTypeApi> {
    caller: Address,
    old_farm_token_amount: GenericTokenAmountPair<M>,
    new_farm_token_amount: GenericTokenAmountPair<M>,
    farm_supply: BigUint<M>,
    reward_token_amount: GenericTokenAmountPair<M>,
    reward_reserve: BigUint<M>,
    old_farm_attributes: FarmTokenAttributes<M>,
    new_farm_attributes: FarmTokenAttributes<M>,
    created_with_merge: bool,
    block: u64,
    epoch: u64,
    timestamp: u64,
}

#[derive(TopEncode)]
pub struct CompoundRewardsEvent<M: ManagedTypeApi> {
    caller: Address,
    old_farm_token_amount: GenericTokenAmountPair<M>,
    new_farm_token_amount: GenericTokenAmountPair<M>,
    farm_supply: BigUint<M>,
    reward_token_amount: GenericTokenAmountPair<M>,
    reward_reserve: BigUint<M>,
    old_farm_attributes: FarmTokenAttributes<M>,
    new_farm_attributes: FarmTokenAttributes<M>,
    created_with_merge: bool,
    block: u64,
    epoch: u64,
    timestamp: u64,
}

#[elrond_wasm::module]
pub trait EventsModule {
    fn emit_enter_farm_event(
        self,
        caller: Address,
        farming_token_amount: FftTokenAmountPair<Self::TypeManager>,
        farming_reserve: BigUint,
        farm_token_amount: GenericTokenAmountPair<Self::TypeManager>,
        farm_supply: BigUint,
        reward_token_reserve: FftTokenAmountPair<Self::TypeManager>,
        farm_attributes: FarmTokenAttributes<Self::TypeManager>,
        created_with_merge: bool,
    ) {
        let epoch = self.blockchain().get_block_epoch();
        self.enter_farm_event(
            caller.clone(),
            farm_token_amount.token_id.clone(),
            farm_attributes.with_locked_rewards,
            epoch,
            EnterFarmEvent {
                caller,
                farming_token_amount,
                farming_reserve,
                farm_token_amount,
                farm_supply,
                reward_token_reserve,
                farm_attributes,
                created_with_merge,
                block: self.blockchain().get_block_nonce(),
                epoch,
                timestamp: self.blockchain().get_block_timestamp(),
            },
        )
    }

    fn emit_exit_farm_event(
        self,
        caller: Address,
        farming_token_amount: FftTokenAmountPair<Self::TypeManager>,
        farming_reserve: BigUint,
        farm_token_amount: GenericTokenAmountPair<Self::TypeManager>,
        farm_supply: BigUint,
        reward_token_amount: GenericTokenAmountPair<Self::TypeManager>,
        reward_reserve: BigUint,
        farm_attributes: FarmTokenAttributes<Self::TypeManager>,
    ) {
        let epoch = self.blockchain().get_block_epoch();
        self.exit_farm_event(
            caller.clone(),
            farm_token_amount.token_id.clone(),
            farm_attributes.with_locked_rewards,
            epoch,
            ExitFarmEvent {
                caller,
                farming_token_amount,
                farming_reserve,
                farm_token_amount,
                farm_supply,
                reward_token_amount,
                reward_reserve,
                farm_attributes,
                block: self.blockchain().get_block_nonce(),
                epoch,
                timestamp: self.blockchain().get_block_timestamp(),
            },
        )
    }

    fn emit_claim_rewards_event(
        self,
        caller: Address,
        old_farm_token_amount: GenericTokenAmountPair<Self::TypeManager>,
        new_farm_token_amount: GenericTokenAmountPair<Self::TypeManager>,
        farm_supply: BigUint,
        reward_token_amount: GenericTokenAmountPair<Self::TypeManager>,
        reward_reserve: BigUint,
        old_farm_attributes: FarmTokenAttributes<Self::TypeManager>,
        new_farm_attributes: FarmTokenAttributes<Self::TypeManager>,
        created_with_merge: bool,
    ) {
        let epoch = self.blockchain().get_block_epoch();
        self.claim_rewards_event(
            caller.clone(),
            old_farm_token_amount.token_id.clone(),
            old_farm_attributes.with_locked_rewards,
            epoch,
            ClaimRewardsEvent {
                caller,
                old_farm_token_amount,
                new_farm_token_amount,
                farm_supply,
                reward_token_amount,
                reward_reserve,
                old_farm_attributes,
                new_farm_attributes,
                created_with_merge,
                block: self.blockchain().get_block_nonce(),
                epoch,
                timestamp: self.blockchain().get_block_timestamp(),
            },
        )
    }

    fn emit_compound_rewards_event(
        self,
        caller: Address,
        old_farm_token_amount: GenericTokenAmountPair<Self::TypeManager>,
        new_farm_token_amount: GenericTokenAmountPair<Self::TypeManager>,
        farm_supply: BigUint,
        reward_token_amount: GenericTokenAmountPair<Self::TypeManager>,
        reward_reserve: BigUint,
        old_farm_attributes: FarmTokenAttributes<Self::TypeManager>,
        new_farm_attributes: FarmTokenAttributes<Self::TypeManager>,
        created_with_merge: bool,
    ) {
        let epoch = self.blockchain().get_block_epoch();
        self.compound_rewards_event(
            caller.clone(),
            old_farm_token_amount.token_id.clone(),
            old_farm_attributes.with_locked_rewards,
            epoch,
            CompoundRewardsEvent {
                caller,
                old_farm_token_amount,
                new_farm_token_amount,
                farm_supply,
                reward_token_amount,
                reward_reserve,
                old_farm_attributes,
                new_farm_attributes,
                created_with_merge,
                block: self.blockchain().get_block_nonce(),
                epoch,
                timestamp: self.blockchain().get_block_timestamp(),
            },
        )
    }

    #[event("enter_farm")]
    fn enter_farm_event(
        self,
        #[indexed] caller: Address,
        #[indexed] farming_token: TokenIdentifier,
        #[indexed] with_locked_rewards: bool,
        #[indexed] epoch: u64,
        enter_farm_event: EnterFarmEvent<Self::TypeManager>,
    );

    #[event("exit_farm")]
    fn exit_farm_event(
        self,
        #[indexed] caller: Address,
        #[indexed] farm_token: TokenIdentifier,
        #[indexed] with_locked_rewards: bool,
        #[indexed] epoch: u64,
        exit_farm_event: ExitFarmEvent<Self::TypeManager>,
    );

    #[event("claim_rewards")]
    fn claim_rewards_event(
        self,
        #[indexed] caller: Address,
        #[indexed] farm_token: TokenIdentifier,
        #[indexed] with_locked_rewards: bool,
        #[indexed] epoch: u64,
        claim_rewards_event: ClaimRewardsEvent<Self::TypeManager>,
    );

    #[event("compound_rewards")]
    fn compound_rewards_event(
        self,
        #[indexed] caller: Address,
        #[indexed] farm_token: TokenIdentifier,
        #[indexed] with_locked_rewards: bool,
        #[indexed] epoch: u64,
        compound_rewards_event: CompoundRewardsEvent<Self::TypeManager>,
    );
}
