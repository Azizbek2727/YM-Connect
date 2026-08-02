package ymconnect.v1;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;

import com.google.protobuf.MessageLite;
import com.google.protobuf.Parser;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;

final class FixtureRoundTripTest {
    private static final Path FIXTURES = Path.of("../../protocol/fixtures/v1").normalize();

    private static <T extends MessageLite> void assertRoundTrip(
            String fileName,
            Parser<T> parser
    ) throws IOException {
        byte[] expected = Files.readAllBytes(FIXTURES.resolve(fileName));
        T decoded = parser.parseFrom(expected);
        assertArrayEquals(expected, decoded.toByteArray());
    }

    @Test
    void goldenFixturesRoundTrip() throws IOException {
        assertRoundTrip("protocol-version.bin", Common.ProtocolVersion.parser());
        assertRoundTrip("connector-hello.bin", Connector.ConnectorHello.parser());
        assertRoundTrip("player-snapshot.bin", Player.PlayerSnapshot.parser());
        assertRoundTrip("command-request.bin", Control.CommandRequest.parser());
        assertRoundTrip("pairing-offer.bin", Session.PairingOffer.parser());
        assertRoundTrip("client-envelope.bin", Session.ClientEnvelope.parser());
    }
}
