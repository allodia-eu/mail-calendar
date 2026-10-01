// Changing folders from the drawer (docs/folder-pane.md, "Changing the tree"): what a row offers,
// where a folder may move, what each dialog says, and what reaches the core.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.rememberDrawerState
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertHasNoClickAction
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.longClick
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.performTouchInput
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AccountFolderRow
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.FolderAction
import uniffi.mailcal_bindings.FolderIntent
import uniffi.mailcal_bindings.FolderNameCheck
import uniffi.mailcal_bindings.FolderNotice
import uniffi.mailcal_bindings.FolderProblem
import uniffi.mailcal_bindings.FolderRole

private fun ctx(): Context = RuntimeEnvironment.getApplication()

// Inbox, Trash holding `Old`, and `Work` holding `2024`, stamped the way the core stamps them.
private fun tree() = listOf(
    folder("inbox", "INBOX", FolderRole.INBOX, acceptsFolders = true, acceptsMessages = true),
    folder("trash", "Trash", FolderRole.TRASH, hasChildren = true, expanded = true, acceptsMessages = true),
    folder("old", "Old", null, parent = "trash", depth = 1u, inTrash = true, editable = true, acceptsMessages = true),
    folder("work", "Work", null, hasChildren = true, expanded = true, editable = true, acceptsFolders = true, acceptsMessages = true),
    folder("w2024", "2024", null, parent = "work", depth = 1u, editable = true, acceptsFolders = true, acceptsMessages = true),
)

// Trash holding `Old` holding `Deep`, where only `Deep` takes a folder; Junk, which takes none and
// holds none; and `Work` holding `2024`.
private fun deepTree() = listOf(
    folder("trash", "Trash", FolderRole.TRASH, hasChildren = true),
    folder("old", "Old", null, parent = "trash", depth = 1u, hasChildren = true, inTrash = true, editable = true),
    folder("deep", "Deep", null, parent = "old", depth = 2u, inTrash = true, editable = true, acceptsFolders = true),
    folder("junk", "Junk", FolderRole.JUNK),
    folder("work", "Work", null, hasChildren = true, editable = true, acceptsFolders = true),
    folder("w2024", "2024", null, parent = "work", depth = 1u, editable = true, acceptsFolders = true),
)

@RunWith(RobolectricTestRunner::class)
class FolderEditingTest {
    @get:Rule val compose = createComposeRule()

    private val sent = mutableListOf<FolderIntent>()
    private val checked = mutableListOf<String>()

    private fun drawer(
        folders: List<AccountFolderRow>,
        notice: FolderNotice? = null,
        open: DrawerValue = DrawerValue.Open,
    ) {
        val editing = FolderEditing(
            dispatch = { sent.add(it) },
            checkName = { _, _, name, _ ->
                checked.add(name)
                when (name) {
                    "" -> FolderNameCheck.EMPTY
                    "Work" -> FolderNameCheck.TAKEN
                    else -> FolderNameCheck.VALID
                }
            },
        )
        compose.setContent {
            FolderDrawerScaffold(
                drawerState = rememberDrawerState(open),
                accounts = listOf(AccountRow(id = "acct", email = "me@work.example", name = "", expanded = true)),
                accountFolders = folders,
                selectedAccount = null,
                selectedFolder = null,
                unifiedUnread = 0u,
                queued = 0,
                showingOutbox = false,
                onSelectAccount = {},
                onSelectFolder = { _, _ -> },
                onSetExpanded = { _, _ -> },
                onSetFolderExpanded = { _, _, _ -> },
                onShowOutbox = {},
                folderEditing = editing,
                folderNotice = notice,
                content = {},
            )
        }
    }

    @Test
    fun `a row offers only what the core stamped on it`() {
        val rows = tree()
        assertEquals(emptyList<FolderMenuItem>(), folderMenu(rows[1]))
        assertEquals(listOf(FolderMenuItem.NEW_FOLDER), folderMenu(rows[0]))
        assertEquals(
            listOf(FolderMenuItem.NEW_FOLDER, FolderMenuItem.RENAME, FolderMenuItem.MOVE, FolderMenuItem.DELETE),
            folderMenu(rows[3]),
        )
        // A pending folder offers nothing, whatever else is set on it.
        assertEquals(emptyList<FolderMenuItem>(), folderMenu(folder("p", "P", null, pending = true, editable = true)))
        assertEquals(listOf(FolderMenuItem.NEW_FOLDER), accountMenu(accountFolderRow("acct", rows, true)))
        assertEquals(emptyList<FolderMenuItem>(), accountMenu(accountFolderRow("acct", rows, false)))
    }

    @Test
    fun `a folder can move to the top or into any folder outside itself but not where it is`() {
        val rows = tree()
        val targets = moveTargets(rows[3], rows, ctx())
        assertEquals(listOf(null, "inbox"), targets.map { it.key })
        assertEquals(L10n.folder_move_top_level(ctx()), targets[0].name)
        assertEquals(L10n.folder_move_top_level(ctx()), targets[0].label)
        // Work is already at the top: Top level stays as the root, but choosing it moves nothing.
        assertEquals(listOf(false, true), targets.map { it.enabled })

        // 2024 already sits in Work, so Work is not offered.
        val fromInside = moveTargets(rows[4], rows, ctx())
        assertEquals(listOf(null, "inbox"), fromInside.map { it.key })
        assertTrue(fromInside.all { it.enabled })
    }

    @Test
    fun `a top-level folder with nowhere to go is offered nothing`() {
        val only = folder("work", "Work", null, editable = true, acceptsFolders = true)
        assertEquals(emptyList<MoveTarget>(), moveTargets(only, listOf(only), ctx()))
    }

    @Test
    fun `a path names a folder with the folders it sits in`() {
        val paths = folderPaths(tree(), ctx())
        assertEquals("Work / 2024", paths["w2024"])
        assertEquals("${L10n.folder_trash(ctx())} / Old", paths["old"])
    }

    @Test
    fun `a picker indents each folder by its depth`() {
        val mail = messageTargets(tree(), showing = null, ctx())
        assertEquals(listOf("inbox", "trash", "old", "work", "w2024"), mail.map { it.key })
        assertEquals(listOf(0, 0, 1, 0, 1), mail.map { it.indent })

        // Under Top level every folder sits one step deeper, as the drawer's tree sits inside its
        // account.
        val folders = moveTargets(tree()[0], tree(), ctx())
        assertEquals(listOf(null, "work", "w2024"), folders.map { it.key })
        assertEquals(listOf(0, 1, 2), folders.map { it.indent })
    }

    @Test
    fun `a picker row shows its own name and is called by its path`() {
        val targets = moveTargets(deepTree()[4], deepTree(), ctx())
        val deep = targets.single { it.key == "deep" }
        assertEquals("Deep", deep.name)
        assertEquals("${L10n.folder_trash(ctx())} / Old / Deep", deep.label)
        assertEquals(L10n.folder_trash(ctx()), targets.single { it.key == "trash" }.name)
        assertEquals(FolderRole.TRASH, targets.single { it.key == "trash" }.role)
    }

    @Test
    fun `a parent that is no destination stays as context and does not respond`() {
        val targets = moveTargets(deepTree()[4], deepTree(), ctx())
        // Junk takes no folder and holds none that does, so it is left out; Trash and Old hold
        // one at any depth, so they stay, disabled, in the drawer's order.
        assertEquals(listOf(null, "trash", "old", "deep"), targets.map { it.key })
        assertEquals(listOf(false, false, false, true), targets.map { it.enabled })
        assertEquals(listOf(0, 1, 2, 3), targets.map { it.indent })
    }

    @Test
    fun `the folder being moved and everything inside it are never listed`() {
        // Moving Old takes Deep with it, so Trash is left holding no destination and goes too.
        val targets = moveTargets(deepTree()[1], deepTree(), ctx())
        assertEquals(listOf(null, "work", "w2024"), targets.map { it.key })
        assertTrue(targets.all { it.enabled })
    }

    @Test
    fun `a message can be filed in any folder that takes mail except the one on screen`() {
        val rows = tree() + listOf(
            // Stamped as the core stamps them: Junk, Drafts and a folder still being made take no
            // mail from a move.
            folder("junk", "Junk", FolderRole.JUNK),
            folder("drafts", "Drafts", FolderRole.DRAFTS),
            folder("p", "P", null, pending = true),
        )
        val targets = messageTargets(rows, showing = "inbox", ctx())
        assertEquals(listOf("trash", "old", "work", "w2024"), targets.map { it.key })
        assertEquals("Work / 2024", targets.last().label)
        assertEquals("2024", targets.last().name)
        // No Top level: mail is filed in a folder, so the folders sit at their own depth.
        assertTrue(targets.none { it.key == null })
        assertEquals(listOf(0, 1, 0, 1), targets.map { it.indent })
        assertEquals(5, messageTargets(rows, showing = null, ctx()).size)
    }

    @Test
    fun `the folder on screen stays in the message picker only as the parent of one`() {
        // Work is on screen and 2024 inside it takes mail, so Work stays, disabled, above it.
        val underWork = messageTargets(tree(), showing = "work", ctx())
        assertEquals(listOf("inbox", "trash", "old", "work", "w2024"), underWork.map { it.key })
        assertEquals(listOf(true, true, true, false, true), underWork.map { it.enabled })

        // 2024 holds nothing, so on screen it is simply absent.
        val leaf = messageTargets(tree(), showing = "w2024", ctx())
        assertEquals(listOf("inbox", "trash", "old", "work"), leaf.map { it.key })
        assertTrue(leaf.all { it.enabled })
    }

    @Test
    fun `the message picker sends the message where it was picked`() {
        val picked = mutableListOf<MoveTarget>()
        val targets = messageTargets(tree(), showing = "inbox", ctx())
        compose.setContent {
            MoveTargetDialog(
                title = L10n.message_move_title(ctx()),
                targets = targets,
                onPick = { picked.add(it) },
                onClose = {},
            )
        }
        compose.onNodeWithText(L10n.message_move_title(ctx())).assertIsDisplayed()
        compose.onNodeWithContentDescription("Work / 2024").performClick()
        assertEquals(listOf("w2024"), picked.map { it.key })
    }

    @Test
    fun `a parent the picker keeps for context cannot be picked`() {
        val picked = mutableListOf<MoveTarget>()
        val trash = L10n.folder_trash(ctx())
        compose.setContent {
            MoveTargetDialog(
                title = L10n.message_move_title(ctx()),
                targets = messageTargets(tree(), showing = "trash", ctx()),
                onPick = { picked.add(it) },
                onClose = {},
            )
        }
        compose.onNodeWithContentDescription(trash)
            .assertIsNotEnabled()
            .assertHasNoClickAction()
            .performClick()
        assertEquals(emptyList<String?>(), picked.map { it.key })

        compose.onNodeWithContentDescription("$trash / Old").assertIsEnabled().performClick()
        assertEquals(listOf("old"), picked.map { it.key })
    }

    @Test
    fun `delete says Trash outside Trash and for good inside it`() {
        val rows = tree()
        assertEquals(L10n.action_move_to_trash(ctx()), deleteCopy(rows[3], ctx()).confirm)
        assertEquals(L10n.folder_delete_title(ctx(), "Work"), deleteCopy(rows[3], ctx()).title)
        assertEquals(L10n.action_delete_permanently(ctx()), deleteCopy(rows[2], ctx()).confirm)
        assertEquals(L10n.folder_delete_permanent_message(ctx()), deleteCopy(rows[2], ctx()).message)
    }

    @Test
    fun `a name field says nothing until something is typed`() {
        assertNull(nameProblem(FolderNameCheck.EMPTY, "", ctx()))
        assertEquals(L10n.folder_name_empty(ctx()), nameProblem(FolderNameCheck.EMPTY, "  ", ctx()))
        assertEquals(L10n.folder_name_taken(ctx()), nameProblem(FolderNameCheck.TAKEN, "Work", ctx()))
        assertNull(nameProblem(FolderNameCheck.VALID, "Receipts", ctx()))
    }

    @Test
    fun `a long press opens the menu and a new folder goes inside the pressed one`() {
        drawer(listOf(accountFolderRow("acct", tree(), true)))

        compose.onNodeWithText("2024").performTouchInput { longClick() }
        compose.onNodeWithText(L10n.folder_action_new(ctx())).performClick()

        val create = compose.onNodeWithText(L10n.action_create(ctx()))
        create.assertIsNotEnabled()
        compose.onNode(SemanticsMatcher.keyIsDefined(SemanticsProperties.EditableText))
            .performTextReplacement("Work")
        compose.onNodeWithText(L10n.folder_name_taken(ctx())).assertIsDisplayed()
        create.assertIsNotEnabled()
        compose.onNode(SemanticsMatcher.keyIsDefined(SemanticsProperties.EditableText))
            .performTextReplacement("Receipts")
        create.assertIsEnabled().performClick()

        assertEquals(listOf<FolderIntent>(FolderIntent.Create("acct", "w2024", "Receipts")), sent)
        assertTrue("the name was checked as it was typed", checked.contains("Work"))
    }

    @Test
    fun `every menu item is a named accessibility action on the row`() {
        drawer(listOf(accountFolderRow("acct", tree(), true)))

        val actions = compose.onNodeWithText("Work").fetchSemanticsNode()
            .config[SemanticsActions.CustomActions].map { it.label }
        assertEquals(
            listOf(
                L10n.folder_action_new(ctx()),
                L10n.folder_action_rename(ctx()),
                L10n.folder_action_move(ctx()),
                L10n.folder_action_delete(ctx()),
            ),
            actions,
        )
    }

    @Test
    fun `move to lists the tree and sends the folder where it was picked`() {
        drawer(listOf(accountFolderRow("acct", tree(), true)))

        compose.onNodeWithText("2024").performTouchInput { longClick() }
        compose.onNodeWithText(L10n.folder_action_move(ctx())).performClick()
        compose.onNodeWithContentDescription(L10n.folder_move_top_level(ctx())).performClick()

        assertEquals(listOf<FolderIntent>(FolderIntent.Move("acct", "w2024", null)), sent)
    }

    @Test
    fun `deleting from inside Trash asks about a permanent delete`() {
        drawer(listOf(accountFolderRow("acct", tree(), true)))

        compose.onNodeWithText("Old").performTouchInput { longClick() }
        compose.onNodeWithText(L10n.folder_action_delete(ctx())).performClick()
        compose.onNodeWithText(L10n.folder_delete_permanent_title(ctx(), "Old")).assertIsDisplayed()
        compose.onNodeWithText(L10n.action_delete_permanently(ctx())).performClick()

        assertEquals(listOf<FolderIntent>(FolderIntent.Delete("acct", "old")), sent)
    }

    @Test
    fun `a pending folder is marked as waiting and offers no menu`() {
        val rows = listOf(folder("pending-folder:3", "Travel", null, pending = true))
        drawer(listOf(accountFolderRow("acct", rows, true)))

        val node = compose.onNodeWithText("Travel").fetchSemanticsNode()
        assertEquals(L10n.folder_pending(ctx()), node.config[SemanticsProperties.StateDescription])
        assertTrue(SemanticsActions.CustomActions !in node.config)
    }

    @Test
    fun `a refused change stands until it is closed`() {
        drawer(
            listOf(accountFolderRow("acct", tree(), true)),
            notice = FolderNotice("acct", "Work", FolderAction.RENAME, FolderProblem.CHANGED_ELSEWHERE),
            // The notice sits over the list, which is what is on screen once the drawer is shut.
            open = DrawerValue.Closed,
        )

        compose.onNodeWithText(L10n.folder_notice_changed(ctx(), "Work")).assertIsDisplayed()
        compose.onNodeWithText(L10n.action_close(ctx())).performClick()

        assertEquals(listOf<FolderIntent>(FolderIntent.DismissNotice), sent)
    }
}
