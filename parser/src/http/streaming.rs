use nom::bytes::streaming::{tag, take};
use nom::character::streaming::digit0;
use nom::{Err::Incomplete, IResult};

#[derive(Debug, Eq, PartialEq)]
pub enum CRLFState {
    NeedCR,
    NeedLF,
    Complete,
}

pub fn crlf(mut i: &[u8], mut state: CRLFState) -> IResult<&[u8], CRLFState> {
    loop {
        match state {
            CRLFState::NeedCR => match tag("\r")(i) {
                Ok((o, _)) => {
                    state = CRLFState::NeedLF;
                    i = o;
                }
                Err(Incomplete(_)) => {
                    break Ok((i, CRLFState::NeedCR));
                }
                Err(e) => {
                    break Err(e);
                }
            },
            CRLFState::NeedLF => {
                match tag("\n")(i) {
                    Ok((o, _)) => {
                        break Ok((o, CRLFState::Complete));
                    }
                    Err(Incomplete(_)) => {
                        break Ok((i, CRLFState::NeedLF));
                    }
                    Err(e) => {
                        break Err(e);
                    }
                };
            }
            _ => {
                break Ok((i, CRLFState::Complete));
            }
        };
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DigitState {
    NeedNumbers(Vec<u8>),
    Complete(Vec<u8>),
}

pub fn digit_stream(i: &[u8], state: DigitState) -> IResult<&[u8], DigitState> {
    match state {
        DigitState::NeedNumbers(mut nums) => match digit0(i) {
            Ok((o, nums_bytes)) => {
                nums.extend_from_slice(nums_bytes);
                Ok((o, DigitState::Complete(nums)))
            }
            Err(Incomplete(_)) => {
                nums.extend_from_slice(i);
                Ok((&i[i.len()..], DigitState::NeedNumbers(nums)))
            }
            Err(e) => Err(e),
        },
        DigitState::Complete(nums) => Ok((i, DigitState::Complete(nums))),
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum TakeIgnore {
    NeedTake(u64),
    Complete,
}

pub fn take_ignore_stream(i: &[u8], state: TakeIgnore) -> IResult<&[u8], TakeIgnore> {
    match state {
        TakeIgnore::NeedTake(size) => match take(size)(i) {
            Ok((o, _)) => Ok((o, TakeIgnore::Complete)),
            Err(Incomplete(_)) => Ok((&i[i.len()..], TakeIgnore::NeedTake(size - i.len() as u64))),
            Err(e) => Err(e),
        },
        TakeIgnore::Complete => Ok((i, TakeIgnore::Complete)),
    }
}

#[test]
fn test_crlf() {
    let b = b"\r\na";
    assert_eq!(crlf(b, CRLFState::NeedCR).unwrap().1, CRLFState::Complete);
    let b = b"\ra";
    assert_eq!(
        crlf(b, CRLFState::NeedCR).unwrap_err(),
        Err::Error(Error::new(&b[1..], ErrorKind::Tag))
    );
    let b = b"\r";
    assert_eq!(crlf(b, CRLFState::NeedCR).unwrap().1, CRLFState::NeedLF);
}

#[test]
fn test_digit() {
    let b = b"111a";
    assert_eq!(
        digit_stream(b, DigitState::NeedNumbers(vec![])).unwrap().1,
        DigitState::Complete(b"111".to_vec())
    );
    let b = b"111";
    assert_eq!(
        digit_stream(b, DigitState::NeedNumbers(vec![])).unwrap(),
        (&b[b.len()..], DigitState::NeedNumbers(b"111".to_vec()))
    );
}

#[test]
fn test_take_ignore_stream() {
    let b = b"111";
    assert_eq!(
        take_ignore_stream(b, TakeIgnore::NeedTake(3)).unwrap(),
        (&b[b.len()..], TakeIgnore::Complete)
    );
    let b = b"111";
    assert_eq!(
        take_ignore_stream(b, TakeIgnore::NeedTake(100)).unwrap(),
        (&b[b.len()..], TakeIgnore::NeedTake(97))
    );
    let b = b"1111111";
    let (_, take_ignore) = take_ignore_stream(&b[..3], TakeIgnore::NeedTake(7)).unwrap();
    assert_eq!(
        take_ignore_stream(&b[3..], take_ignore).unwrap(),
        (&b[b.len()..], TakeIgnore::Complete)
    )
}
