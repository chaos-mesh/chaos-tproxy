use crate::parser::http::header::HeaderField;
use nom::lib::std::str::from_utf8;
use nom::bytes::streaming::{take, take_while1, take_while, tag};
use nom::character::is_digit;
use nom::multi::many_till;
use nom::{Err::{self, Incomplete}, error::{Error, ErrorKind}, Needed, IResult, Compare, CompareResult};
use crate::parser::http::streaming::*;
use nom::error::ParseError;


#[derive(Debug, Eq, PartialEq)]
pub enum ChunkState {
    Size(DigitState),
    CRLFFirst(u64, CRLFState),
    Take(u64, TakeIgnore),
    CRLFEnd(u64, CRLFState),
    Complete(u64),
}

pub fn chunk(mut i: &[u8], mut state: ChunkState) -> IResult<&[u8], ChunkState> {
    if let ChunkState::Size(digit_state) = state {
        match digit_stream(i, digit_state) {
            Ok((o, DigitState::Complete(nums))) => {
                if nums.is_empty() {
                    return Err(Err::Error(
                        Error::from_error_kind(
                            i, ErrorKind::Eof,
                        )
                    ));
                };
                if nums.len() > 16 {
                    return Err(Err::Error(
                        Error::from_error_kind(
                            i, ErrorKind::TooLarge,
                        )
                    ));
                }
                i = o;
                state = ChunkState::CRLFFirst(
                    match u64::from_str_radix(from_utf8(nums.as_slice()).unwrap(), 16) {
                        Ok(v) => { v }
                        Err(e) => {
                            return Err(Err::Error(
                                Error::from_error_kind(
                                    i, ErrorKind::Digit,
                                )
                            ));
                        }
                    },
                    CRLFState::NeedCR,
                );
            }
            Ok((o, DigitState::NeedNumbers(nums))) => {
                return Ok((o, ChunkState::Size(DigitState::NeedNumbers(nums))));
            }
            Err(e) => { return Err(e); }
        }
    };
    if let ChunkState::CRLFFirst(size, crlf_state) = state {
        match crlf(i, crlf_state) {
            Ok((o, CRLFState::NeedCR)) => {
                return Ok((o, ChunkState::CRLFFirst(size, CRLFState::NeedCR)));
            }
            Ok((o, CRLFState::NeedLF)) => {
                return Ok((o, ChunkState::CRLFFirst(size, CRLFState::NeedLF)));
            }
            Ok((o, CRLFState::Complete)) => {
                i = o;
                state = ChunkState::Take(size, TakeIgnore::NeedTake(size));
            }
            Err(e) => { return Err(e); }
        }
    };
    if let ChunkState::Take(size, take_state) = state {
        match take_ignore_stream(i, take_state) {
            Ok((o, TakeIgnore::NeedTake(num))) => {
                return Ok((o, ChunkState::Take(num, TakeIgnore::NeedTake(num))));
            }
            Ok((o, TakeIgnore::Complete)) => {
                i = o;
                state = ChunkState::CRLFEnd(size, CRLFState::NeedCR);
            }
            Err(e) => { return Err(e); }
        }
    };
    if let ChunkState::CRLFEnd(size, crlf_state) = state {
        match crlf(i, crlf_state) {
            Ok((o, CRLFState::NeedCR)) => {
                return Ok((o, ChunkState::CRLFEnd(size, CRLFState::NeedCR)));
            }
            Ok((o, CRLFState::NeedLF)) => {
                return Ok((o, ChunkState::CRLFEnd(size, CRLFState::NeedLF)));
            }
            Ok((o, CRLFState::Complete)) => {
                i = o;
                state = ChunkState::Complete(size);
            }
            Err(e) => { return Err(e); }
        }
    };
    Ok((i, state))
}

#[derive(Debug, Eq, PartialEq)]
pub enum ChunkedBodyState {
    Chunks(ChunkState),
    Complete,
}

pub fn chunked_body(mut i: &[u8], mut state: ChunkedBodyState) -> IResult<&[u8], ChunkedBodyState> {
    loop {
        match state {
            ChunkedBodyState::Chunks(chunk_state) => {
                match chunk(i, chunk_state) {
                    Ok((o, ChunkState::Complete(size))) => {
                        if size == 0 {
                            break Ok((o, ChunkedBodyState::Complete));
                        } else {
                            state = ChunkedBodyState::Chunks(ChunkState::Size(DigitState::NeedNumbers(vec![])));
                            i = o;
                        }
                    }
                    Ok((o, chunk_state)) => {
                        break Ok((i, ChunkedBodyState::Chunks(chunk_state)));
                    }
                    Err(e) => { return Err(e); }
                }
            }
            ChunkedBodyState::Complete => {
                break Ok((i, ChunkedBodyState::Complete));
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum BodyState {
    TakeContentLength(TakeIgnore),
    ChunkedBody(ChunkedBodyState),
    NoLengthInfo,
    FaultLengthInfo,
    Complete
}

pub fn body_state(header_fields: &Vec<HeaderField>) -> BodyState {
    let result = match header_fields.iter().find(|&header_field|
        header_field.field_name.compare_no_case(b"Transfer-Encoding") == CompareResult::Ok) {
        None => {
            BodyState::NoLengthInfo
        }
        Some(field) => {
            if from_utf8(field.field_value).unwrap().contains("chunked") {
                return BodyState::ChunkedBody(ChunkedBodyState::Chunks(
                    ChunkState::Size(
                        DigitState::NeedNumbers(
                            vec![]
                        )
                    )
                ));
            } else {
                BodyState::FaultLengthInfo
            }
        }
    };
    let result = match header_fields.iter().find(|&header_field|
        header_field.field_name.compare_no_case(b"Content-Length") == CompareResult::Ok) {
        None => {
            if result != BodyState::FaultLengthInfo {
                BodyState::NoLengthInfo
            } else {
                BodyState::FaultLengthInfo
            }
        }
        Some(field) => {
            match u64::from_str_radix(from_utf8(field.field_value).unwrap(), 10) {
                Ok(size) => { return BodyState::TakeContentLength(TakeIgnore::NeedTake(size)); }
                Err(e) => { BodyState::FaultLengthInfo }
            }
        }
    };
    result
}

pub fn body(i: &[u8], body_state: BodyState) -> IResult<&[u8],BodyState> {
    match body_state {
        BodyState::NoLengthInfo => {
            println!("No Length Info");
            Ok((i, BodyState::Complete))
        }
        BodyState::ChunkedBody(chunk_body_state) => {
            println!("Chunked Body");
            return match chunked_body(i, chunk_body_state) {
                Ok((o, ChunkedBodyState::Complete)) => {
                    Ok((o, BodyState::Complete))
                }
                Ok((o, state)) => {
                    Ok((o, BodyState::ChunkedBody(state)))
                }
                Err(e) => { Err(e) }
            }
        }
        BodyState::TakeContentLength(take_ignore_state) => {
            println!("Take Content Length");
            return match take_ignore_stream(i,take_ignore_state) {
                Ok((o, TakeIgnore::Complete)) => {
                    Ok((o, BodyState::Complete))
                }
                Ok((o, state)) => {
                    Ok((o, BodyState::TakeContentLength(state)))
                }
                Err(e) => { Err(e) }
            }
        }
        BodyState::FaultLengthInfo => {
            println!("Fault Length Info");
            Ok((i, BodyState::Complete))
        }
        BodyState::Complete => {
            println!("Body Complete");
            Ok((i, BodyState::Complete))
        }
    }
}

#[test]
fn test_chunk() {
    let b = b"5\r\n11111\r\n";
    assert_eq!(chunk(&b[..], ChunkState::Size(DigitState::NeedNumbers(vec![]))).unwrap().1,
               ChunkState::Complete(5));
    let b = b"5\r\n11111\r\n0\r\n\r\naaaa";
    assert_eq!(chunked_body(&b[..], ChunkedBodyState::Chunks(
        ChunkState::Size(DigitState::NeedNumbers(vec![]))
    )).unwrap(),
               (&b[15..], ChunkedBodyState::Complete)
    )
}

#[test]
fn test_body() {
    let b = b"5\r\n11111\r\n0\r\n\r\naaaa";
    assert_eq!(body(&b[..], BodyState::ChunkedBody(ChunkedBodyState::Chunks(
        ChunkState::Size(
            DigitState::NeedNumbers(
                vec![]
            )
        )
    ))).unwrap(),(&b[15..], BodyState::Complete));
    let b = b"5\r\n11111\r\n0\r\n\r\naaaa";
    assert_eq!(body(&b[..], BodyState::TakeContentLength(TakeIgnore::NeedTake(14))).unwrap(),
               (&b[14..], BodyState::Complete));
    let b = b"5\r\n11111\r\n0\r\n\r\naaaa";
    assert_eq!(body(&b[..], BodyState::FaultLengthInfo).unwrap(),(&b[..], BodyState::Complete));
    let b = b"5\r\n11111\r\n0\r\n\r\naaaa";
    assert_eq!(body(&b[..], BodyState::NoLengthInfo).unwrap_err(),Err::Incomplete(Needed::Unknown));
}