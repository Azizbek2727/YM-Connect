package ymconnect.v1

import java.nio.file.Files
import java.nio.file.Path
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

class FixtureRoundTripTest {
    private val fixtures: Path = Path.of("../../protocol/fixtures/v1").normalize()

    @Test
    fun `builder creates canonical protocol version`() {
        val version = protocolVersion {
            major = 1
            minor = 0
            patch = 0
        }
        assertEquals(1, version.major)
        assertContentEquals(
            Files.readAllBytes(fixtures.resolve("protocol-version.bin")),
            version.toByteArray(),
        )
    }

    @Test
    fun `client envelope fixture round trips`() {
        val expected = Files.readAllBytes(fixtures.resolve("client-envelope.bin"))
        val decoded = Session.ClientEnvelope.parseFrom(expected)
        assertContentEquals(expected, decoded.toByteArray())
    }
}
