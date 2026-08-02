use prost::Message;
use ym_connect_protocol::v1::{
    ClientEnvelope, CommandRequest, ConnectorHello, PairingOffer, PlayerSnapshot, ProtocolVersion,
};

fn round_trip<M>(bytes: &[u8]) -> Result<(), prost::DecodeError>
where
    M: Message + Default,
{
    let decoded = M::decode(bytes)?;
    assert_eq!(decoded.encode_to_vec(), bytes);
    Ok(())
}

#[test]
fn golden_fixtures_round_trip() -> Result<(), prost::DecodeError> {
    round_trip::<ProtocolVersion>(include_bytes!(
        "../../../protocol/fixtures/v1/protocol-version.bin"
    ))?;
    round_trip::<ConnectorHello>(include_bytes!(
        "../../../protocol/fixtures/v1/connector-hello.bin"
    ))?;
    round_trip::<PlayerSnapshot>(include_bytes!(
        "../../../protocol/fixtures/v1/player-snapshot.bin"
    ))?;
    round_trip::<CommandRequest>(include_bytes!(
        "../../../protocol/fixtures/v1/command-request.bin"
    ))?;
    round_trip::<PairingOffer>(include_bytes!(
        "../../../protocol/fixtures/v1/pairing-offer.bin"
    ))?;
    round_trip::<ClientEnvelope>(include_bytes!(
        "../../../protocol/fixtures/v1/client-envelope.bin"
    ))?;
    Ok(())
}
