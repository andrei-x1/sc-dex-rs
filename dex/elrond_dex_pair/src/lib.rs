#![no_std]

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

const DEFAULT_TRANSFER_EXEC_GAS_LIMIT: u64 = 35000000;
const DEFAULT_EXTERN_SWAP_GAS_LIMIT: u64 = 50000000;

mod amm;
pub mod config;
mod events;
pub mod fee;
mod liquidity_pool;

use common_structs::FftTokenAmountPair;
use config::State;

type AddLiquidityResultType<ManagedTypeApi> = MultiResult3<
    FftTokenAmountPair<ManagedTypeApi>,
    FftTokenAmountPair<ManagedTypeApi>,
    FftTokenAmountPair<ManagedTypeApi>,
>;

type RemoveLiquidityResultType<ManagedTypeApi> =
    MultiResult2<FftTokenAmountPair<ManagedTypeApi>, FftTokenAmountPair<ManagedTypeApi>>;

type SwapTokensFixedInputResultType<ManagedTypeApi> = FftTokenAmountPair<ManagedTypeApi>;

type SwapTokensFixedOutputResultType<ManagedTypeApi> =
    MultiResult2<FftTokenAmountPair<ManagedTypeApi>, FftTokenAmountPair<ManagedTypeApi>>;

#[elrond_wasm::contract]
pub trait Pair:
    amm::AmmModule
    + fee::FeeModule
    + liquidity_pool::LiquidityPoolModule
    + config::ConfigModule
    + token_supply::TokenSupplyModule
    + token_send::TokenSendModule
    + events::EventsModule
{
    #[init]
    fn init(
        &self,
        first_token_id: TokenIdentifier,
        second_token_id: TokenIdentifier,
        router_address: Address,
        router_owner_address: Address,
        total_fee_percent: u64,
        special_fee_percent: u64,
    ) -> SCResult<()> {
        require!(
            first_token_id.is_valid_esdt_identifier(),
            "First token ID is not a valid ESDT identifier"
        );
        require!(
            second_token_id.is_valid_esdt_identifier(),
            "Second token ID is not a valid ESDT identifier"
        );
        require!(
            first_token_id != second_token_id,
            "Exchange tokens cannot be the same"
        );
        let lp_token_id = self.lp_token_identifier().get();
        require!(
            first_token_id != lp_token_id,
            "First token ID cannot be the same as LP token ID"
        );
        require!(
            second_token_id != lp_token_id,
            "Second token ID cannot be the same as LP token ID"
        );
        self.try_set_fee_percents(total_fee_percent, special_fee_percent)?;

        self.state().set_if_empty(&State::ActiveNoSwaps);
        self.transfer_exec_gas_limit()
            .set_if_empty(&DEFAULT_TRANSFER_EXEC_GAS_LIMIT);
        self.extern_swap_gas_limit()
            .set_if_empty(&DEFAULT_EXTERN_SWAP_GAS_LIMIT);

        self.router_address().set(&router_address);
        self.router_owner_address().set(&router_owner_address);
        self.first_token_id().set(&first_token_id);
        self.second_token_id().set(&second_token_id);
        Ok(())
    }

    #[payable("*")]
    #[endpoint(addLiquidity)]
    fn add_liquidity(
        &self,
        first_token_amount_min: BigUint,
        second_token_amount_min: BigUint,
        #[var_args] opt_accept_funds_func: OptionalArg<BoxedBytes>,
    ) -> SCResult<AddLiquidityResultType<Self::TypeManager>> {
        require!(self.is_active(), "Not active");
        require!(
            !self.lp_token_identifier().is_empty(),
            "LP token not issued"
        );

        let payments = self.call_value().get_all_esdt_transfers();
        require!(payments.len() == 2, "Bad payments len");

        let expected_first_token_id = self.first_token_id().get();
        require!(
            payments[0].token_name == expected_first_token_id,
            "Bad first token id"
        );
        let first_token_amount_desired = payments[0].amount.clone();
        require!(
            first_token_amount_desired > 0,
            "Insufficient first token funds sent"
        );
        require!(
            first_token_amount_desired >= first_token_amount_min,
            "Input first token desired amount is lower than minimul"
        );

        let expected_second_token_id = self.second_token_id().get();
        require!(
            payments[1].token_name == expected_second_token_id,
            "Bad second token id"
        );
        let second_token_amount_desired = payments[1].amount.clone();
        require!(
            second_token_amount_desired > 0,
            "Insufficient second token funds sent"
        );
        require!(
            second_token_amount_desired >= second_token_amount_min,
            "Input second token desired amount is lower than minimul"
        );

        let old_k = self.calculate_k_for_reserves();
        let (first_token_amount, second_token_amount) = self.calculate_optimal_amounts(
            &first_token_amount_desired,
            &second_token_amount_desired,
            &first_token_amount_min,
            &second_token_amount_min,
        )?;

        let liquidity =
            self.pool_add_liquidity(first_token_amount.clone(), second_token_amount.clone())?;

        let caller = self.blockchain().get_caller();
        let first_token_unused = &first_token_amount_desired - &first_token_amount;
        let second_token_unused = &second_token_amount_desired - &second_token_amount;

        // Once liquidity has been added, the new K should always be greater than the old K.
        let new_k = self.calculate_k_for_reserves();
        self.validate_k_invariant_strict(&old_k, &new_k)?;

        let lp_token_id = self.lp_token_identifier().get();
        self.mint_tokens(&lp_token_id, &liquidity);

        let lp_payment = EsdtTokenPayment::from(lp_token_id.clone(), 0, liquidity.clone());
        let first_unused_payment = EsdtTokenPayment::from(
            expected_first_token_id.clone(),
            0,
            first_token_unused.clone(),
        );
        let second_unused_payment = EsdtTokenPayment::from(
            expected_second_token_id.clone(),
            0,
            second_token_unused.clone(),
        );
        self.send_multiple_tokens_compact(
            &[lp_payment, first_unused_payment, second_unused_payment],
            &caller,
            &opt_accept_funds_func,
        )?;

        let lp_token_amount = FftTokenAmountPair {
            token_id: lp_token_id,
            amount: liquidity,
        };
        let first_token_amount = FftTokenAmountPair {
            token_id: expected_first_token_id.clone(),
            amount: first_token_amount,
        };
        let second_token_amount = FftTokenAmountPair {
            token_id: expected_second_token_id.clone(),
            amount: second_token_amount,
        };
        let first_token_reserve = FftTokenAmountPair {
            token_id: expected_first_token_id.clone(),
            amount: self.pair_reserve(&expected_first_token_id).get(),
        };
        let second_token_reserve = FftTokenAmountPair {
            token_id: expected_second_token_id.clone(),
            amount: self.pair_reserve(&expected_second_token_id).get(),
        };
        self.emit_add_liquidity_event(
            caller,
            first_token_amount.clone(),
            second_token_amount.clone(),
            lp_token_amount.clone(),
            self.get_total_lp_token_supply(),
            [first_token_reserve, second_token_reserve].to_vec(),
        );
        Ok((lp_token_amount, first_token_amount, second_token_amount).into())
    }

    #[payable("*")]
    #[endpoint(removeLiquidity)]
    fn remove_liquidity(
        &self,
        #[payment_token] token_id: TokenIdentifier,
        #[payment_amount] liquidity: BigUint,
        first_token_amount_min: BigUint,
        second_token_amount_min: BigUint,
        #[var_args] opt_accept_funds_func: OptionalArg<BoxedBytes>,
    ) -> SCResult<RemoveLiquidityResultType<Self::TypeManager>> {
        require!(
            !self.lp_token_identifier().is_empty(),
            "LP token not issued"
        );

        let caller = self.blockchain().get_caller();
        let lp_token_id = self.lp_token_identifier().get();
        require!(token_id == lp_token_id, "Wrong liquidity token");

        let old_k = self.calculate_k_for_reserves();
        let (first_token_amount, second_token_amount) = self.pool_remove_liquidity(
            liquidity.clone(),
            first_token_amount_min,
            second_token_amount_min,
        )?;

        let first_token_id = self.first_token_id().get();
        let second_token_id = self.second_token_id().get();

        // Once liquidity has been removed, the new K should always be lesser than the old K.
        let new_k = self.calculate_k_for_reserves();
        self.validate_k_invariant_strict(&new_k, &old_k)?;

        let first_token_payment =
            EsdtTokenPayment::from(first_token_id.clone(), 0, first_token_amount.clone());
        let second_token_payment =
            EsdtTokenPayment::from(second_token_id.clone(), 0, second_token_amount.clone());
        self.send_multiple_tokens_compact(
            &[first_token_payment, second_token_payment],
            &caller,
            &opt_accept_funds_func,
        )?;
        self.burn_tokens(&token_id, &liquidity);

        let lp_token_amount = FftTokenAmountPair {
            token_id: lp_token_id,
            amount: liquidity,
        };
        let first_token_amount = FftTokenAmountPair {
            token_id: first_token_id.clone(),
            amount: first_token_amount,
        };
        let second_token_amount = FftTokenAmountPair {
            token_id: second_token_id.clone(),
            amount: second_token_amount,
        };
        let first_token_reserve = FftTokenAmountPair {
            token_id: first_token_id.clone(),
            amount: self.pair_reserve(&first_token_id).get(),
        };
        let second_token_reserve = FftTokenAmountPair {
            token_id: second_token_id.clone(),
            amount: self.pair_reserve(&second_token_id).get(),
        };
        self.emit_remove_liquidity_event(
            caller,
            first_token_amount.clone(),
            second_token_amount.clone(),
            lp_token_amount,
            self.get_total_lp_token_supply(),
            [first_token_reserve, second_token_reserve].to_vec(),
        );
        Ok((first_token_amount, second_token_amount).into())
    }

    #[payable("*")]
    #[endpoint(removeLiquidityAndBuyBackAndBurnToken)]
    fn remove_liquidity_and_burn_token(
        &self,
        #[payment_token] token_in: TokenIdentifier,
        #[payment_amount] amount_in: BigUint,
        token_to_buyback_and_burn: TokenIdentifier,
    ) -> SCResult<()> {
        let caller = self.blockchain().get_caller();
        self.require_whitelisted(&caller)?;

        require!(
            !self.lp_token_identifier().is_empty(),
            "LP token not issued"
        );
        require!(
            token_in == self.lp_token_identifier().get(),
            "Wrong liquidity token"
        );

        let first_token_id = self.first_token_id().get();
        let second_token_id = self.second_token_id().get();

        let first_token_min_amount = self.types().big_uint_from(1u64);
        let second_token_min_amount = self.types().big_uint_from(1u64);
        let (first_token_amount, second_token_amount) = self.pool_remove_liquidity(
            amount_in.clone(),
            first_token_min_amount,
            second_token_min_amount,
        )?;

        let dest_address = Address::zero();
        self.send_fee_slice(
            &first_token_id,
            &first_token_amount,
            &dest_address,
            &token_to_buyback_and_burn,
            &first_token_id,
            &second_token_id,
        );
        self.send_fee_slice(
            &second_token_id,
            &second_token_amount,
            &dest_address,
            &token_to_buyback_and_burn,
            &first_token_id,
            &second_token_id,
        );
        self.burn_tokens(&token_in, &amount_in);

        Ok(())
    }

    #[payable("*")]
    #[endpoint(swapNoFeeAndForward)]
    fn swap_no_fee(
        &self,
        #[payment_token] token_in: TokenIdentifier,
        #[payment_amount] amount_in: BigUint,
        token_out: TokenIdentifier,
        destination_address: Address,
    ) -> SCResult<()> {
        let caller = self.blockchain().get_caller();
        self.require_whitelisted(&caller)?;

        require!(self.can_swap(), "Swap is not enabled");
        require!(amount_in > 0, "Zero input");

        let first_token_id = self.first_token_id().get();
        let second_token_id = self.second_token_id().get();
        require!(token_in != token_out, "Cannot swap same token");
        require!(
            token_in == first_token_id || token_in == second_token_id,
            "Invalid token in"
        );
        require!(
            token_out == first_token_id || token_out == second_token_id,
            "Invalid token out"
        );

        let old_k = self.calculate_k_for_reserves();

        let amount_out =
            self.swap_safe_no_fee(&first_token_id, &second_token_id, &token_in, &amount_in);
        require!(amount_out > 0, "Zero output");

        // A swap should not decrease the value of K. Should either be greater or equal.
        let new_k = self.calculate_k_for_reserves();
        self.validate_k_invariant(&old_k, &new_k)?;

        self.send_fee_or_burn_on_zero_address(&token_out, &amount_out, &destination_address);

        let swap_out_token_amount = FftTokenAmountPair {
            token_id: token_out,
            amount: amount_out,
        };
        self.emit_swap_no_fee_and_forward_event(caller, swap_out_token_amount, destination_address);
        Ok(())
    }

    #[payable("*")]
    #[endpoint(swapTokensFixedInput)]
    fn swap_tokens_fixed_input(
        &self,
        #[payment_token] token_in: TokenIdentifier,
        #[payment_amount] amount_in: BigUint,
        token_out: TokenIdentifier,
        amount_out_min: BigUint,
        #[var_args] opt_accept_funds_func: OptionalArg<BoxedBytes>,
    ) -> SCResult<SwapTokensFixedInputResultType<Self::TypeManager>> {
        require!(self.can_swap(), "Swap is not enabled");
        require!(amount_in > 0, "Invalid amount_in");
        require!(token_in != token_out, "Swap with same token");
        let first_token_id = self.first_token_id().get();
        let second_token_id = self.second_token_id().get();
        require!(
            token_in == first_token_id || token_in == second_token_id,
            "Invalid token in"
        );
        require!(
            token_out == first_token_id || token_out == second_token_id,
            "Invalid token out"
        );
        let old_k = self.calculate_k_for_reserves();

        let mut reserve_token_out = self.pair_reserve(&token_out).get();
        require!(
            reserve_token_out > amount_out_min,
            "Insufficient reserve for token out"
        );

        let mut reserve_token_in = self.pair_reserve(&token_in).get();
        let amount_out_optimal =
            self.get_amount_out(&amount_in, &reserve_token_in, &reserve_token_out);
        require!(
            amount_out_optimal >= amount_out_min,
            "Computed amount out lesser than minimum amount out"
        );
        require!(
            reserve_token_out > amount_out_optimal,
            "Insufficient amount out reserve"
        );
        require!(amount_out_optimal != 0, "Optimal value is zero");

        let caller = self.blockchain().get_caller();

        let mut fee_amount = self.types().big_uint_zero();
        let mut amount_in_after_fee = amount_in.clone();
        if self.is_fee_enabled() {
            fee_amount = self.get_special_fee_from_input(&amount_in);
            amount_in_after_fee -= &fee_amount;
        }

        reserve_token_in += &amount_in_after_fee;
        reserve_token_out -= &amount_out_optimal;
        self.update_reserves(&reserve_token_in, &reserve_token_out, &token_in, &token_out);

        // A swap should not decrease the value of K. Should either be greater or equal.
        let new_k = self.calculate_k_for_reserves();
        self.validate_k_invariant(&old_k, &new_k)?;

        //The transaction was made. We are left with $(fee) of $(token_in) as fee.
        if self.is_fee_enabled() {
            self.send_fee(&token_in, &fee_amount);
        }
        self.send_tokens(
            &token_out,
            &amount_out_optimal,
            &caller,
            &opt_accept_funds_func,
        )?;

        let token_amount_in = FftTokenAmountPair {
            token_id: token_in.clone(),
            amount: amount_in,
        };
        let token_amount_out = FftTokenAmountPair {
            token_id: token_out.clone(),
            amount: amount_out_optimal,
        };
        let token_in_reserves = FftTokenAmountPair {
            token_id: token_in,
            amount: reserve_token_in,
        };
        let token_out_reserves = FftTokenAmountPair {
            token_id: token_out,
            amount: reserve_token_out,
        };
        self.emit_swap_event(
            caller,
            token_amount_in,
            token_amount_out.clone(),
            fee_amount,
            [token_in_reserves, token_out_reserves].to_vec(),
        );
        Ok(token_amount_out)
    }

    #[payable("*")]
    #[endpoint(swapTokensFixedOutput)]
    fn swap_tokens_fixed_output(
        &self,
        #[payment_token] token_in: TokenIdentifier,
        #[payment_amount] amount_in_max: BigUint,
        token_out: TokenIdentifier,
        amount_out: BigUint,
        #[var_args] opt_accept_funds_func: OptionalArg<BoxedBytes>,
    ) -> SCResult<SwapTokensFixedOutputResultType<Self::TypeManager>> {
        require!(self.can_swap(), "Swap is not enabled");
        require!(amount_in_max > 0, "Invalid amount_in");
        require!(token_in != token_out, "Invalid swap with same token");
        let first_token_id = self.first_token_id().get();
        let second_token_id = self.second_token_id().get();
        require!(
            token_in == first_token_id || token_in == second_token_id,
            "Invalid token in"
        );
        require!(
            token_out == first_token_id || token_out == second_token_id,
            "Invalid token out"
        );
        require!(amount_out != 0, "Desired amount out cannot be zero");
        let old_k = self.calculate_k_for_reserves();

        let mut reserve_token_out = self.pair_reserve(&token_out).get();
        require!(
            reserve_token_out > amount_out,
            "Insufficient reserve for token out"
        );

        let mut reserve_token_in = self.pair_reserve(&token_in).get();
        let amount_in_optimal =
            self.get_amount_in(&amount_out, &reserve_token_in, &reserve_token_out);
        require!(
            amount_in_optimal <= amount_in_max,
            "Computed amount in greater than maximum amount in"
        );

        let caller = self.blockchain().get_caller();
        let residuum = &amount_in_max - &amount_in_optimal;

        let mut fee_amount = self.types().big_uint_zero();
        let mut amount_in_optimal_after_fee = amount_in_optimal.clone();
        if self.is_fee_enabled() {
            fee_amount = self.get_special_fee_from_input(&amount_in_optimal);
            amount_in_optimal_after_fee -= &fee_amount;
        }

        reserve_token_in += &amount_in_optimal_after_fee;
        reserve_token_out -= &amount_out;
        self.update_reserves(&reserve_token_in, &reserve_token_out, &token_in, &token_out);

        // A swap should not decrease the value of K. Should either be greater or equal.
        let new_k = self.calculate_k_for_reserves();
        self.validate_k_invariant(&old_k, &new_k)?;

        //The transaction was made. We are left with $(fee) of $(token_in) as fee.
        if self.is_fee_enabled() {
            self.send_fee(&token_in, &fee_amount);
        }

        let token_out_payment = EsdtTokenPayment::from(token_out.clone(), 0, amount_out.clone());
        let residuum_payment = EsdtTokenPayment::from(token_in.clone(), 0, amount_out.clone());
        self.send_multiple_tokens_compact(
            &[token_out_payment, residuum_payment],
            &caller,
            &opt_accept_funds_func,
        )?;

        let token_amount_in = FftTokenAmountPair {
            token_id: token_in.clone(),
            amount: amount_in_optimal,
        };
        let token_amount_out = FftTokenAmountPair {
            token_id: token_out.clone(),
            amount: amount_out,
        };
        let token_in_reserves = FftTokenAmountPair {
            token_id: token_in.clone(),
            amount: reserve_token_in,
        };
        let token_out_reserves = FftTokenAmountPair {
            token_id: token_out,
            amount: reserve_token_out,
        };
        let residuum_token_amount = FftTokenAmountPair {
            token_id: token_in,
            amount: residuum,
        };
        self.emit_swap_event(
            caller,
            token_amount_in,
            token_amount_out.clone(),
            fee_amount,
            [token_in_reserves, token_out_reserves].to_vec(),
        );
        Ok((token_amount_out, residuum_token_amount).into())
    }

    fn send_tokens(
        &self,
        token: &TokenIdentifier,
        amount: &BigUint,
        destination: &Address,
        opt_accept_funds_func: &OptionalArg<BoxedBytes>,
    ) -> SCResult<()> {
        if amount > &0 {
            self.send_fft_tokens(token, amount, destination, opt_accept_funds_func)?;
        }
        Ok(())
    }

    #[endpoint(setLpTokenIdentifier)]
    fn set_lp_token_identifier(&self, token_identifier: TokenIdentifier) -> SCResult<()> {
        self.require_permissions()?;
        require!(self.lp_token_identifier().is_empty(), "LP token not empty");
        require!(
            token_identifier != self.first_token_id().get()
                && token_identifier != self.second_token_id().get(),
            "LP token should differ from the exchange tokens"
        );
        require!(
            token_identifier.is_valid_esdt_identifier(),
            "Provided identifier is not a valid ESDT identifier"
        );

        self.lp_token_identifier().set(&token_identifier);

        Ok(())
    }

    #[inline]
    fn validate_k_invariant(&self, lower: &BigUint, greater: &BigUint) -> SCResult<()> {
        require!(lower <= greater, "K invariant failed");
        Ok(())
    }

    #[inline]
    fn validate_k_invariant_strict(&self, lower: &BigUint, greater: &BigUint) -> SCResult<()> {
        require!(lower < greater, "K invariant failed");
        Ok(())
    }

    #[view(getTokensForGivenPosition)]
    fn get_tokens_for_given_position(
        &self,
        liquidity: BigUint,
    ) -> MultiResult2<FftTokenAmountPair<Self::TypeManager>, FftTokenAmountPair<Self::TypeManager>>
    {
        self.get_both_tokens_for_given_position(liquidity)
    }

    #[view(getReservesAndTotalSupply)]
    fn get_reserves_and_total_supply(&self) -> MultiResult3<BigUint, BigUint, BigUint> {
        let first_token_id = self.first_token_id().get();
        let second_token_id = self.second_token_id().get();
        let first_token_reserve = self.pair_reserve(&first_token_id).get();
        let second_token_reserve = self.pair_reserve(&second_token_id).get();
        let total_supply = self.get_total_lp_token_supply();
        (first_token_reserve, second_token_reserve, total_supply).into()
    }

    #[view(getAmountOut)]
    fn get_amount_out_view(
        &self,
        token_in: TokenIdentifier,
        amount_in: BigUint,
    ) -> SCResult<BigUint> {
        require!(amount_in > 0, "Zero input");

        let first_token_id = self.first_token_id().get();
        let second_token_id = self.second_token_id().get();
        let first_token_reserve = self.pair_reserve(&first_token_id).get();
        let second_token_reserve = self.pair_reserve(&second_token_id).get();

        if token_in == first_token_id {
            require!(second_token_reserve > 0, "Zero reserves for second token");
            let amount_out =
                self.get_amount_out(&amount_in, &first_token_reserve, &second_token_reserve);
            require!(
                second_token_reserve > amount_out,
                "Not enough reserves for second token"
            );
            Ok(amount_out)
        } else if token_in == second_token_id {
            require!(first_token_reserve > 0, "Zero reserves for first token");
            let amount_out =
                self.get_amount_out(&amount_in, &second_token_reserve, &first_token_reserve);
            require!(
                first_token_reserve > amount_out,
                "Not enough reserves first token"
            );
            Ok(amount_out)
        } else {
            sc_error!("Not a known token")
        }
    }

    #[view(getAmountIn)]
    fn get_amount_in_view(
        &self,
        token_wanted: TokenIdentifier,
        amount_wanted: BigUint,
    ) -> SCResult<BigUint> {
        require!(amount_wanted > 0, "Zero input");

        let first_token_id = self.first_token_id().get();
        let second_token_id = self.second_token_id().get();
        let first_token_reserve = self.pair_reserve(&first_token_id).get();
        let second_token_reserve = self.pair_reserve(&second_token_id).get();

        if token_wanted == first_token_id {
            require!(
                first_token_reserve > amount_wanted,
                "Not enough reserves for first token"
            );
            let amount_in =
                self.get_amount_in(&amount_wanted, &second_token_reserve, &first_token_reserve);
            Ok(amount_in)
        } else if token_wanted == second_token_id {
            require!(
                second_token_reserve > amount_wanted,
                "Not enough reserves for second token"
            );
            let amount_in =
                self.get_amount_in(&amount_wanted, &first_token_reserve, &second_token_reserve);
            Ok(amount_in)
        } else {
            sc_error!("Not a known token")
        }
    }

    #[view(getEquivalent)]
    fn get_equivalent(&self, token_in: TokenIdentifier, amount_in: BigUint) -> SCResult<BigUint> {
        require!(amount_in > 0, "Zero input");
        let zero = self.types().big_uint_zero();

        let first_token_id = self.first_token_id().get();
        let second_token_id = self.second_token_id().get();
        let first_token_reserve = self.pair_reserve(&first_token_id).get();
        let second_token_reserve = self.pair_reserve(&second_token_id).get();
        if first_token_reserve == 0 || second_token_reserve == 0 {
            return Ok(zero);
        }

        if token_in == first_token_id {
            Ok(self.quote(&amount_in, &first_token_reserve, &second_token_reserve))
        } else if token_in == second_token_id {
            Ok(self.quote(&amount_in, &second_token_reserve, &first_token_reserve))
        } else {
            sc_error!("Not a known token")
        }
    }

    #[inline]
    fn is_active(&self) -> bool {
        let state = self.state().get();
        state == State::Active || state == State::ActiveNoSwaps
    }

    #[inline]
    fn can_swap(&self) -> bool {
        self.state().get() == State::Active
    }
}
