package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.compose.runtime.Composable
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.share.InviteContent
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Les invitations, l'interface (docs/invitations.md ; textes de docs/link-ui.md, « Invitations »), en français :
 * « Ajouter {ordinateur} » après un code d'invitation, le champ « Mot de passe », « Ajouter l'ordinateur » inactif tant
 * que le champ est vide, « Annuler », « Ajout de {ordinateur}… » pendant l'appel, le refus sous le champ ; au
 * téléphone et au téléviseur.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class, qualifiers = "fr-w914dp-h411dp-land-xxhdpi")
class InviteUiTest {
    @get:Rule val rule = createComposeRule()

    private fun show(form: FormFactor, content: @Composable () -> Unit) = rule.setContent { KnTheme(form, motion = false) { content() } }

    @Test
    fun theInvitation_asksForThePassword() {
        val sent = mutableListOf<String>()
        var cancelled = 0
        show(FormFactor.PHONE) { InviteContent("PC du salon", busy = false, error = null, onCancel = { cancelled++ }, onAdd = { sent += it }) }
        rule.onNodeWithText("Ajouter PC du salon").assertExists()
        rule.onNodeWithText("Cette invitation ajoute l'ordinateur à vos appareils. Entrez le mot de passe donné avec le code.").assertExists()
        rule.onNodeWithText("Mot de passe").assertExists()
        // Vide : le bouton attend.
        rule.onNodeWithText("Ajouter l'ordinateur").assertIsNotEnabled()
        rule.onNode(hasSetTextAction()).performTextInput("k7mq-p2xa-9fz3")
        rule.onNodeWithText("Ajouter l'ordinateur").assertIsEnabled().performClick()
        assertEquals(listOf("k7mq-p2xa-9fz3"), sent)
        rule.onNodeWithText("Annuler").performClick()
        assertEquals(1, cancelled)
    }

    @Test
    fun atTheTv_theSameScreen() {
        show(FormFactor.TV) { InviteContent("PC du salon", busy = false, error = null, onCancel = {}, onAdd = {}) }
        rule.onNodeWithText("Ajouter PC du salon").assertExists()
        rule.onNodeWithText("Ajouter l'ordinateur").assertIsNotEnabled()
        rule.onNodeWithText("Annuler").assertExists()
    }

    @Test
    fun withoutAName_theComputer() {
        show(FormFactor.PHONE) { InviteContent("", busy = true, error = null, onCancel = {}, onAdd = {}) }
        // Le titre et le bouton.
        rule.onAllNodesWithText("Ajouter l'ordinateur").assertCountEquals(2)
        rule.onNodeWithText("Ajout de l'ordinateur…").assertExists()
    }

    @Test
    fun whileAdding_andARefusalUnderTheField() {
        show(FormFactor.PHONE) { InviteContent("PC du salon", busy = true, error = "Mot de passe refusé.", onCancel = {}, onAdd = {}) }
        rule.onNodeWithText("Ajout de PC du salon…").assertExists()
        rule.onNodeWithText("Mot de passe refusé.").assertExists()
    }
}
