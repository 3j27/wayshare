//! hp_answer <offer.bplist> <answer.bplist> [ssrc]: answer an AVCMediaStreamNegotiator offer.
//! HP_SIZE=WxH sets the video stream size (default 1920x1080).
use anyhow::{Context, Result};
use wayshare::hp::negotiation::{AnswerParams, Offer, answer};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let [_, offer_path, answer_path, rest @ ..] = args.as_slice() else {
        anyhow::bail!("usage: hp_answer <offer> <answer> [ssrc]");
    };
    let ssrc = match rest.first() {
        Some(s) => s.parse().context("ssrc")?,
        None => rand::random(),
    };
    let offer = Offer::parse(&std::fs::read(offer_path)?)?;
    let stream_size = match std::env::var("HP_SIZE") {
        Ok(v) => {
            let (w, h) = v.split_once('x').context("HP_SIZE=WxH")?;
            (w.parse()?, h.parse()?)
        }
        Err(_) => (1920, 1080),
    };
    let a = answer(
        &offer,
        &AnswerParams {
            ssrc,
            local: None,
            stream_size,
        },
    )?;
    std::fs::write(answer_path, &a.bytes)?;
    println!(
        "mode={} ssrc={} remote_ssrc={} pt={} tiles={}",
        offer.mode, a.ssrc, a.remote_ssrc, a.payload_type, a.tiles
    );
    Ok(())
}
