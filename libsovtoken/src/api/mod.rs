//
// export public methods
//
//
//
#![allow(unused_variables)]
#![allow(unused_imports)]
#[warn(unused_imports)]

use std;
use std::ffi::CString;
use std::thread;

use libc::c_char;
use indy::payments::Payment;
use indy::ErrorCode;
use logic::address::*;
use logic::payments::{CreatePaymentSDK, CreatePaymentHandler};

use logic::config::{
    payment_config::{PaymentRequest},
    general::{InputConfig, OutputConfig},
    output_mint_config::{MintRequest},
    payment_address_config::{PaymentAddressConfig},
    set_fees_config::{SetFeesRequest, Fees},
};
use logic::request::Request;
use serde_json;
use utils::ffi_support::{str_from_char_ptr, cstring_from_str, string_from_char_ptr, deserialize_from_char_ptr, c_pointer_from_string};
use utils::json_conversion::JsonDeserialize;
use utils::general::ResultExtension;
use utils::types::*;
use utils::validation::{validate_did_len};

type JsonCallback = Option<extern fn(command_handle: i32, err: ErrorCode, json_pointer: *const c_char) -> ErrorCode>;


/// # Description
/// This method generates private part of payment address
/// and stores it in a secure place. Ideally it should be
/// secret in libindy wallet (see crypto module).
///
/// Note that payment method should be able to resolve this
/// secret by fully resolvable payment address format.
///
/// from tokens-interface.md/CreatePaymentAddressCB
///
/// # Params
/// command_handle: command handle to map callback to context
/// config_str: payment address config as json:
///   {
///     seed: <str>, // allows deterministic creation of payment address
///   }
/// cb: description
///
/// # Returns
/// on Success:  payment_address will have the format:
///              pay:sov:{32 byte public key}{4 digit check sum}
///
/// # Errors
/// description of errors
#[no_mangle]
pub extern "C" fn create_payment_address_handler(command_handle: i32,
                                                 wallet_handle: i32,
                                                 config_str: *const c_char,
                                                 cb: JsonCallback) -> ErrorCode {
    if cb.is_none() {
        return ErrorCode::CommonInvalidParam4;
    }

    if config_str.is_null() {
        return ErrorCode::CommonInvalidParam2
    }

    let json_config_str: String = match string_from_char_ptr(config_str) {
        Some(s) => s,
        None => return ErrorCode::CommonInvalidParam2
    };

    // indy-sdk accepts { } for valid seed info to create a key.  Serde deseralization does not
    // like { } as valid.  if we get any kind of serialization failure assume we can use the default
    let config: PaymentAddressConfig = match PaymentAddressConfig::from_json(&json_config_str) {
        Ok(c) => c,
        Err(_) => PaymentAddressConfig { seed : "".to_string()},
    };

    thread::spawn(move || {
        // to return both payment address and private key pair so that we can write the private
        // key into the ledger
        let handler = CreatePaymentHandler::new(CreatePaymentSDK {} );
        match handler.create_payment_address(wallet_handle, config) {
            Ok(payment_address) => {
                debug!("create_payment_address_handler returning payment address of '{}'", &payment_address);
                let payment_address_cstring = cstring_from_str(payment_address);
                let payment_address_ptr = payment_address_cstring.as_ptr();

                match cb {
                    Some(f) => f(command_handle, ErrorCode::Success, payment_address_ptr),
                    None => {
                        error!("cb was null even after check");
                        ErrorCode::CommonInvalidState
                    },
                };

            },
            Err(e) => {
                match cb {
                    Some(f) => f(command_handle, ErrorCode::CommonInvalidState, std::ptr::null()),
                    None => {
                        error!("cb was null even after check");
                        ErrorCode::CommonInvalidState
                    },
                };

            },
        };
    });


    return ErrorCode::Success;
}

/// Description
/// call made to wallet to list payment addresses
///    * missing from Slava
///
/// #Params
/// param1: description.
///
/// #Returns
/// description. example if json, etc...
///
/// #Errors
/// description of errors
#[no_mangle]
pub extern "C" fn list_payment_addresses_handler() -> ErrorCode {
    return ErrorCode::Success;
}

/// Description
///
///
/// from tokens-interface.md/AddRequestFeesCB
/// #Params
/// param1: description.
///
/// #Returns
/// description. example if json, etc...
///
/// #Errors
/// description of errors
#[no_mangle]
pub extern "C" fn add_request_fees_handler(command_handle: i32,
                                           wallet_handle: i32,
                                           did: *const c_char, // TODO: Need to remove.
                                           req_json: *const c_char,
                                           inputs_json: *const c_char,
                                           outputs_json: *const c_char,
                                           cb: Option<extern fn(command_handle_: i32,
                                                               err: ErrorCode,
                                                               req_with_fees_json: *const c_char) -> ErrorCode>) -> ErrorCode {


    check_useful_c_callback!(cb, ErrorCode::CommonInvalidParam6);

    trace!("Converting request_json pointer to string");
    let request_json = match string_from_char_ptr(req_json) {
        Some(s) => s,
        None => {
            error!("Failed to convert request_json pointer to string");
            return ErrorCode::CommonInvalidParam3;
        }
    };

    trace!("Converting request_json pointer to string");
    let inputs_json = match string_from_char_ptr(inputs_json) {
        Some(s) => s,
        None => {
            error!("Failed to convert inputs_json pointer to string");
            return ErrorCode::CommonInvalidParam4;
        }
    };

    trace!("Converting request_json pointer to string");
    let outputs_json = match string_from_char_ptr(outputs_json) {
        Some(s) => s,
        None => {
            error!("Failed to convert outputs_json pointer to string.");
            return ErrorCode::CommonInvalidParam5;
        }
    };

    trace!("Converting request_json to serde::json::Value");
    let request_json: serde_json::Value = match serde_json::from_str(&request_json) {
        Ok(value) => value,
        Err(_) => {
            error!("request_json was invalid json.");
            return ErrorCode::CommonInvalidStructure;
        }
    };

    let transaction_type = match request_json.get("type") {
        Some(txn_type) => txn_type,
        None => {
            error!("request_json didn't contain a transaction type.");
            return ErrorCode::CommonInvalidStructure;
        }
    };

    debug!("Request transaction type was: {}", transaction_type);

    /*
        Errors when the request is a XFER request becaause the 
        fees should be implicit in the operation's inputs and
        outputs.
    */
    if transaction_type == "10000" {
        error!("Can't add fees to a transfer request");
        return ErrorCode::CommonInvalidStructure;
    }

    cb(command_handle, ErrorCode::CommonInvalidState, req_json);

    return ErrorCode::Success;
}

/// Description
///
///
/// from tokens-interface.md/ParseResponseWithFeesCB
/// #Params
/// param1: description.
///
/// #Returns
/// description. example if json, etc...
///
/// #Errors
/// description of errors
#[no_mangle]
pub extern "C" fn parse_response_with_fees_handler(command_handle: i32,
                                                   req_json: *const c_char,
                                                   cb: Option<extern fn(command_handle_: i32,
                                                               err: ErrorCode,
                                                               utxo_json: *const c_char) -> ErrorCode>) -> ErrorCode {
    return ErrorCode::Success;
}


/// Description
///
///
/// from tokens-interface.md/BuildPaymentReqCB
/// #Params
/// param1: description.
///
/// #Returns
/// description. example if json, etc...
///
/// #Errors
/// description of errors
#[no_mangle]
pub extern "C" fn build_payment_req_handler(command_handle: i32,
                                            wallet_handle: i32,
                                            submitter_did: *const c_char,
                                            inputs_json: *const c_char,
                                            outputs_json: *const c_char,
                                            cb: Option<extern fn(command_handle_: i32,
                                                        err: ErrorCode,
                                                        payment_req_json: *const c_char) -> ErrorCode>) -> ErrorCode {


    let handle_result = api_result_handler!(< *const c_char >, command_handle, cb);

    if cb.is_none() {
        return handle_result(Err(ErrorCode::CommonInvalidParam5));
    }
    if submitter_did.is_null() {
       return handle_result(Err(ErrorCode::CommonInvalidParam2));
    }

    let outputs_config = match deserialize_from_char_ptr::<OutputConfig>(outputs_json) {
        Ok(c) => c,
        Err(e) => return handle_result(Err(e))
    };

    let inputs_config = match deserialize_from_char_ptr::<InputConfig>(inputs_json) {
        Ok(c) => c,
        Err(e) => return handle_result(Err(e))
    };

    let payment_request = PaymentRequest::from_config(outputs_config,inputs_config);
    let payment_request = payment_request.serialize_to_cstring().unwrap();

    return handle_result(Ok(payment_request.as_ptr()));

}

/// Description
///
///
/// from tokens-interface.md/ParsePaymentResponseCB
/// #Params
/// param1: description.
///
/// #Returns
/// description. example if json, etc...
///
/// #Errors
/// description of errors
#[no_mangle]
pub extern "C" fn parse_payment_response_handler(command_handle: i32,
                                                 resp_json: *const c_char,
                                                 cb: Option<extern fn(command_handle_: i32,
                                                             err: ErrorCode,
                                                             utxo_json: *const c_char) -> ErrorCode>) -> ErrorCode {

    return ErrorCode::Success;
}


/// Description
///
///
/// from tokens-interface.md/BuildGetUTXORequestCB
/// #Params
/// param1: description.
///
/// #Returns
/// description. example if json, etc...
///
/// #Errors
/// description of errors
#[no_mangle]
pub extern "C" fn build_get_utxo_request_handler(command_handle: i32,
                                                 wallet_handle: i32,
                                                 submitter_did: *const c_char,
                                                 payment_address: *const c_char,
                                                 cb: JsonCallback)-> ErrorCode {

    check_useful_c_callback!(cb, ErrorCode::CommonInvalidParam5);
    // * C_CHAR to &str
    let submitter_did = str_from_char_ptr(submitter_did).unwrap();
    let payment_address = str_from_char_ptr(payment_address).unwrap();
    // Helper Vars
    let add_len = payment_address.len();

    // validation
    if !validate_did_len(submitter_did) {
        return ErrorCode::CommonInvalidParam3;
    }

    if let Err(e) = validate_address(String::from(payment_address)) {
        return e;
    }

    // start the CBs
    return match Payment::build_get_utxo_request(wallet_handle, submitter_did, payment_address) {
        Ok((txn_req, ..)) => {
            cb(command_handle, ErrorCode::Success, c_pointer_from_string(txn_req));
            ErrorCode::Success
        },
        Err(e) => e
    };
}

/// Description
///
///
///
/// from tokens-interface.md/ParseGetUTXOResponseCB
/// #Params
/// param1: description.
///
/// #Returns
/// description. example if json, etc...
///
/// #Errors
/// description of errors
#[no_mangle]
pub extern "C" fn parse_get_utxo_response_handler(command_handle: i32,
                                                  resp_json: *const c_char,
                                                  cb: Option<extern fn(command_handle_: i32,
                                                                       err: ErrorCode,
                                                                       utxo_json: *const c_char) -> ErrorCode>)-> ErrorCode {
    return ErrorCode::Success;
}

/// Description
///
///
/// from tokens-interface.md/BuildSetTxnFeesReqCB
/// #Params
/// param1: description.
///
/// #Returns
/// description. example if json, etc...
///
/// #Errors
/// description of errors
#[no_mangle]
pub extern "C" fn build_set_txn_fees_handler(command_handle: i32,
                                         wallet_handle: i32,
                                         submitter_did: *const c_char,
                                         fees_json: *const c_char,
                                         cb: Option<extern fn(command_handle_: i32, err: ErrorCode, set_txn_fees_json: *const c_char) -> ErrorCode>) -> ErrorCode {

    let handle_result = |result: Result<*const c_char, ErrorCode>| {
        let result_error_code = result.and(Ok(ErrorCode::Success)).ok_or_err();
        if cb.is_some() {
            let json_pointer = result.unwrap_or(std::ptr::null());
            cb.unwrap()(command_handle, result_error_code, json_pointer);
        }
        return result_error_code;
    };

    if cb.is_some() == false {
        return ErrorCode::CommonInvalidParam3;
    }

    let fees_json_str : &str = match str_from_char_ptr(fees_json) {
        Some(s) => s,
        None => return handle_result(Err(ErrorCode::CommonInvalidParam2))
    };

    let fees_config: Fees = match Fees::from_json(fees_json_str) {
        Ok(c) => c,
        Err(_) => return handle_result(Err(ErrorCode::CommonInvalidStructure))
    };

    let fees_request = SetFeesRequest::from_fee_config(fees_config);
    let fees_request = fees_request.serialize_to_cstring().unwrap();

    return handle_result(Ok(fees_request.as_ptr()));
}

/// Description
///
///
/// from tokens-interface.md/BuildGetTxnFeesReqCB
/// # Params
/// param1: description.
///
/// # Returns
/// description. example if json, etc...
///
/// # Errors
/// description of errors
#[no_mangle]
pub extern "C" fn build_get_txn_fees_handler(command_handle: i32,
                                             wallet_handle: i32,
                                             submitter_did: *const c_char,
                                             cb: Option<extern fn(command_handle_: i32, err: ErrorCode, get_txn_fees_json: *const c_char) -> ErrorCode>) -> ErrorCode {
    return ErrorCode::Success;
}

/// Description
///
///
/// from tokens-interface.md/ParseGetTxnFeesResponseCB
/// # Params
/// param1: description.
///
/// # Returns
/// description. example if json, etc...
///
/// # Errors
/// description of errors
#[no_mangle]
pub extern "C" fn parse_get_txn_fees_response_handler(command_handle: i32,
                                                      resp_json: *const c_char,
                                                      cb: Option<extern fn(command_handle_: i32,
                                                                err: ErrorCode,
                                                                fees_json: *const c_char) -> ErrorCode>)-> ErrorCode {
    return ErrorCode::Success;
}


/// Builds a Mint Request to mint tokens
#[no_mangle]
pub extern "C" fn build_mint_txn_handler(
    command_handle:i32,
    wallet_handle: i32,
    submitter_did: *const c_char,
    outputs_json: *const c_char,
    cb: JsonCallback) -> ErrorCode
{

    let handle_result = api_result_handler!(< *const c_char >, command_handle, cb);

    if cb.is_none() {
        return handle_result(Err(ErrorCode::CommonInvalidParam5));
    }

    let outputs_config = match deserialize_from_char_ptr::<OutputConfig>(outputs_json) {
        Ok(c) => c,
        Err(e) => return handle_result(Err(e))
    };

    let mint_request = MintRequest::from_config(outputs_config);
    let mint_request = mint_request.serialize_to_cstring().unwrap();

    return handle_result(Ok(mint_request.as_ptr()));
}

/**
    exported method indy-sdk will call for us to register our payment methods with indy-sdk

    # Params
    none

    # Returns
    ErrorCode from register_payment_method
*/
#[no_mangle]
pub extern fn sovtoken_init() -> ErrorCode {

    super::utils::logger::init_log();

    debug!("sovtoken_init() started");
    let result = match Payment::register(
        "pay:sov:",
        create_payment_address_handler,
        add_request_fees_handler,
        parse_response_with_fees_handler,
        build_get_utxo_request_handler,
        parse_get_utxo_response_handler,
        build_payment_req_handler,
        parse_payment_response_handler,
        build_mint_txn_handler,
        build_set_txn_fees_handler,
        build_get_txn_fees_handler,
        parse_get_txn_fees_response_handler
    ) {
        Ok(()) => ErrorCode::Success ,
        Err(e) => e ,
    };

    debug!("sovtoken_init() returning {:?}", result);
    return result;
}
