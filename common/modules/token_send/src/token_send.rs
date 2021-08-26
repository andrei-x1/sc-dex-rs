#![no_std]

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use common_structs::Nonce;

#[elrond_wasm::module]
pub trait TokenSendModule {
    fn send_fft_tokens(
        &self,
        token: &TokenIdentifier,
        amount: &BigUint,
        destination: &Address,
        opt_accept_funds_func: &OptionalArg<BoxedBytes>,
    ) -> SCResult<()> {
        let (function, gas_limit) = match opt_accept_funds_func {
            OptionalArg::Some(accept_funds_func) => (
                accept_funds_func.as_slice(),
                self.transfer_exec_gas_limit().get(),
            ),
            OptionalArg::None => {
                let no_func: &[u8] = &[];
                (no_func, 0u64)
            }
        };

        SCResult::from_result(self.send().direct_esdt_execute(
            destination,
            token,
            amount,
            gas_limit,
            function,
            &ArgBuffer::new(),
        ))
    }

    fn send_nft_tokens(
        &self,
        token: &TokenIdentifier,
        nonce: Nonce,
        amount: &BigUint,
        destination: &Address,
        opt_accept_funds_func: &OptionalArg<BoxedBytes>,
    ) -> SCResult<()> {
        let (function, gas_limit) = match opt_accept_funds_func {
            OptionalArg::Some(accept_funds_func) => (
                accept_funds_func.as_slice(),
                self.transfer_exec_gas_limit().get(),
            ),
            OptionalArg::None => {
                let no_func: &[u8] = &[];
                (no_func, 0u64)
            }
        };

        SCResult::from_result(self.send().direct_esdt_nft_execute(
            destination,
            token,
            nonce,
            amount,
            gas_limit,
            function,
            &ArgBuffer::new(),
        ))
    }

    fn send_multiple_tokens(
        &self,
        payments: &[EsdtTokenPayment<Self::TypeManager>],
        destination: &Address,
        opt_accept_funds_func: &OptionalArg<BoxedBytes>,
    ) -> SCResult<()> {
        let (function, gas_limit) = match opt_accept_funds_func {
            OptionalArg::Some(accept_funds_func) => (
                accept_funds_func.as_slice(),
                self.transfer_exec_gas_limit().get(),
            ),
            OptionalArg::None => {
                let no_func: &[u8] = &[];
                (no_func, 0u64)
            }
        };

        SCResult::from_result(self.send().direct_multi_esdt_transfer_execute(
            destination,
            payments,
            gas_limit,
            function,
            &ArgBuffer::new(),
        ))
    }

    fn send_multiple_tokens_compact(
        &self,
        payments: &[EsdtTokenPayment<Self::TypeManager>],
        destination: &Address,
        opt_accept_funds_func: &OptionalArg<BoxedBytes>,
    ) -> SCResult<()> {
        let mut compact_payments = Vec::<EsdtTokenPayment<Self::TypeManager>>::new();
        for payment in payments.iter() {
            if payment.amount != 0 {
                let existing_index = compact_payments.iter().position(|x| {
                    x.token_name == payment.token_name && x.token_nonce == payment.token_nonce
                });

                match existing_index {
                    Some(index) => compact_payments[index].amount += &payment.amount,
                    None => compact_payments.push(payment.clone()),
                }
            }
        }

        let len = compact_payments.len();
        if len == 1 {
            let payment = &compact_payments[0];

            if payment.token_nonce == 0 {
                self.send_fft_tokens(
                    &payment.token_name,
                    &payment.amount,
                    destination,
                    opt_accept_funds_func,
                )
            } else {
                self.send_nft_tokens(
                    &payment.token_name,
                    payment.token_nonce,
                    &payment.amount,
                    destination,
                    opt_accept_funds_func,
                )
            }
        } else if len > 1 {
            self.send_multiple_tokens(&compact_payments, destination, opt_accept_funds_func)
        } else {
            Ok(())
        }
    }

    #[view(getTransferExecGasLimit)]
    #[storage_mapper("transfer_exec_gas_limit")]
    fn transfer_exec_gas_limit(&self) -> SingleValueMapper<Self::Storage, u64>;
}
