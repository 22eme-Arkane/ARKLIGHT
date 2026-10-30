//! Commandes d'ARKLIGHT (mêmes identifiants que `web/js/commands.js`) : celles qu'un contrôleur
//! MIDI peut déclencher (correspondance « Commande d'ARKLIGHT ») et, plus tard, l'éditeur de
//! raccourcis.

use crate::AppWindow;
use slint::SharedString;
use slint::ComponentHandle as _;

/// Une commande : identifiant, libellés français et anglais.
pub struct Command {
    pub id: &'static str,
    pub fr: &'static str,
    pub en: &'static str,
}

const fn c(id: &'static str, fr: &'static str, en: &'static str) -> Command {
    Command { id, fr, en }
}

pub const COMMANDS: &[Command] = &[
    c("projects.open", "Écran des projets…", "Projects screen…"),
    c("scene.new", "Nouveau projet", "New project"),
    c("scene.open", "Ouvrir un projet…", "Open a project…"),
    c("scene.openFile", "Ouvrir un fichier projet…", "Open a project file…"),
    c("scene.save", "Enregistrer", "Save"),
    c("scene.saveAs", "Enregistrer sous…", "Save as…"),
    c("scene.revert", "Revenir à la version enregistrée", "Revert to the saved version"),
    c("scene.rename", "Renommer le projet", "Rename the project"),
    c("projects.folder", "Dossier des projets…", "Projects folder…"),
    c("plans.open", "Séquences et plans…", "Sequences and shots…"),
    c("plan.next", "Plan suivant", "Next shot"),
    c("plan.prev", "Plan précédent", "Previous shot"),
    c("plan.new", "Nouveau plan (copie du plan actuel)", "New shot (copy of the current one)"),
    c("sequence.new", "Nouvelle séquence", "New sequence"),
    c("edit.undo", "Annuler", "Undo"),
    c("edit.redo", "Rétablir", "Redo"),
    c("edit.cut", "Couper", "Cut"),
    c("edit.copy", "Copier", "Copy"),
    c("edit.paste", "Coller", "Paste"),
    c("edit.duplicate", "Dupliquer", "Duplicate"),
    c("edit.delete", "Supprimer", "Delete"),
    c("select.all", "Tout sélectionner", "Select all"),
    c("select.none", "Désélectionner", "Deselect"),
    c("select.invert", "Inverser la sélection", "Invert selection"),
    c("select.same", "Sélectionner les mêmes modèles", "Select the same models"),
    c("group.create", "Grouper la sélection", "Group the selection"),
    c("fixture.power", "Allumer / éteindre la sélection", "Turn the selection on / off"),
    c("fixture.aim", "Viser le personnage", "Aim at the actor"),
    c("fixture.drop", "Poser au sol", "Put on the floor"),
    c("fixture.number", "Numéroter les fixtures…", "Number the fixtures…"),
    c("dmx.readdress", "Ré-adressage automatique…", "Automatic readdressing…"),
    c("tool.translate", "Outil Déplacer", "Move tool"),
    c("tool.rotate", "Outil Tourner (pan / tilt)", "Rotate tool (pan / tilt)"),
    c("snap.toggle", "Aimant (10 cm / 5°)", "Snap (10 cm / 5°)"),
    c("view.persp", "Vue 3D (perspective)", "3D view (perspective)"),
    c("view.front", "Vue de face", "Front view"),
    c("view.top", "Vue de dessus", "Top view"),
    c("view.side", "Vue de côté", "Side view"),
    c("view.frame", "Cadrer la sélection", "Frame the selection"),
    c("view.camera", "Voir par la caméra", "Look through the camera"),
    c("view.highlight", "Surbrillance (voir tout dans le noir)", "Highlight (see everything in the dark)"),
    c("view.reset", "Revenir à la vue de départ", "Back to the starting view"),
    c("view.plot", "Plan 2D / vue 3D", "2D plot / 3D view"),
    c("plot.panel", "Fond et cartouche du plan…", "Plot background and title block…"),
    c("plot.print", "Imprimer / PDF (plan + liste + patch)", "Print / PDF (plot + list + patch)"),
    c("plot.png", "Exporter le plan en image PNG", "Export the plot as a PNG image"),
    c("plot.svg", "Exporter le plan en SVG (vectoriel)", "Export the plot as SVG (vector)"),
    c("plot.equipment", "Liste du matériel (CSV)", "Equipment list (CSV)"),
    c("plot.patch", "Patch DMX (CSV)", "DMX patch (CSV)"),
    c("plot.mvr", "Exporter en MVR (Blackout, consoles…)", "Export as MVR (Blackout, consoles…)"),
    c("dmx.sequence", "Adresser la sélection à la suite…", "Address the selection in sequence…"),
    c("labels.toggle", "Noms des projecteurs", "Fixture names"),
    c("labels.patch", "Adresses DMX", "DMX addresses"),
    c("env.floor", "Sol", "Floor"),
    c("env.cyc", "Cyclo", "Cyc"),
    c("env.stands", "Pieds de projecteur", "Fixture stands"),
    c("view.bloom", "Halo des sources lumineuses", "Glow of light sources"),
    c("view.blackout", "Blackout général", "General blackout"),
    c("drawer.console", "Console (panneau du bas)", "Console (bottom panel)"),
    c("remote.toggle", "Télécommande infrarouge", "Infrared remote"),
    c("drawer.dmx", "Moniteur DMX", "DMX monitor"),
    c("drawer.log", "Journal", "Log"),
    c("drawer.network", "Réseau (Art-Net, sACN)", "Network (Art-Net, sACN)"),
    c("drawer.bluetooth", "Bluetooth (fausse AsteraBox)", "Bluetooth (fake AsteraBox)"),
    c("drawer.assistant", "Assistant IA", "AI assistant"),
    c("midi.open", "Contrôleur MIDI…", "MIDI controller…"),
    c("library.search", "Rechercher dans la bibliothèque", "Search the library"),
    c("library.lights", "Bibliothèque : lumières", "Library: lights"),
    c("library.stands", "Bibliothèque : pieds", "Library: stands"),
    c("library.grip", "Bibliothèque : grip", "Library: grip"),
    c("library.decor", "Bibliothèque : décor", "Library: set"),
    c("console.full", "Console : plein feu sur la sélection", "Console: full on the selection"),
    c("console.black", "Console : noir sur la sélection", "Console: black on the selection"),
    c("console.releaseAll", "Console : tout relâcher", "Console: release all"),
    c("panel.left", "Replier / déplier la bibliothèque", "Fold / unfold the library"),
    c("panel.right", "Replier / déplier l’inspecteur", "Fold / unfold the inspector"),
    c("panel.bottom", "Replier / déplier le panneau du bas", "Fold / unfold the bottom panel"),
    c("view.fullscreen", "Plein écran", "Full screen"),
    c("menu.focus", "Ouvrir la barre de menus", "Open the menu bar"),
    c("view.fullview", "Vue 3D / plan en plein écran", "3D view / plot full screen"),
    c("view.help", "Aide en bas de la vue", "Help at the bottom of the view"),
    c("view.fps", "Images par seconde (i/s)", "Frames per second (fps)"),
    c("console.release", "Relâcher la console", "Release the console"),
    c("ai.voice", "Parler à l’assistant (ORAKLE, maintenir)", "Talk to the assistant (ORAKLE, hold)"),
    c("ai.drawing", "Réinterpréter un plan dessiné…", "Reinterpret a drawn plan…"),
    c("shortcuts.open", "Raccourcis clavier…", "Keyboard shortcuts…"),
    c("app.about", "À propos d’ARKLIGHT…", "About ARKLIGHT…"),
    c("app.support", "Faire un don…", "Donate…"),
    c("app.update", "Rechercher une mise à jour…", "Check for updates…"),
    c("app.news", "Nouveautés d’ARKLIGHT…", "What’s new in ARKLIGHT…"),
    c("app.source", "Code source (GitHub)", "Source code (GitHub)"),
    c("app.issue", "Signaler un problème", "Report a problem"),
    c("app.quit", "Quitter", "Quit"),
];

/// Lance une commande (l'état de l'application ne doit pas être emprunté) ; faux si inconnue.
pub fn run(app: &AppWindow, id: &str) -> bool {
    match id {
        "projects.open" => app.global::<crate::App>().invoke_projects("open".into(), 0),
        "scene.new" => app.global::<crate::App>().invoke_new_project(),
        "scene.open" | "scene.openFile" => app.global::<crate::App>().invoke_open_project(),
        "scene.save" => app.global::<crate::App>().invoke_save_project(),
        "scene.saveAs" => app.global::<crate::App>().invoke_save_project_as(),
        "scene.revert" => app.global::<crate::App>().invoke_action("revert".into()),
        "scene.rename" => {
            let g = app.global::<crate::App>();
            g.set_focus_name(g.get_focus_name() + 1);
        }
        "projects.folder" => app.global::<crate::App>().invoke_projects("folder".into(), 0),
        "plans.open" => app.global::<crate::App>().invoke_plans_action("show".into(), "".into(), "".into()),
        "plan.next" => app.global::<crate::App>().invoke_step_plan(true),
        "plan.prev" => app.global::<crate::App>().invoke_step_plan(false),
        "plan.new" => app.global::<crate::App>().invoke_new_plan("copy".into()),
        "sequence.new" => app.global::<crate::App>().invoke_new_sequence(),
        "edit.undo" => app.global::<crate::App>().invoke_undo(),
        "edit.redo" => app.global::<crate::App>().invoke_redo(),
        "edit.cut" => app.global::<crate::App>().invoke_action("cut".into()),
        "edit.copy" => app.global::<crate::App>().invoke_action("copy".into()),
        "edit.paste" => app.global::<crate::App>().invoke_action("paste".into()),
        "edit.duplicate" => app.global::<crate::App>().invoke_duplicate(),
        "edit.delete" => app.global::<crate::App>().invoke_delete_selection(),
        "select.all" => app.global::<crate::App>().invoke_select_all(),
        "select.none" => app.global::<crate::App>().invoke_select_none(),
        "select.invert" => app.global::<crate::App>().invoke_action("invert".into()),
        "select.same" => app.global::<crate::App>().invoke_action("select-same".into()),
        "group.create" => app.global::<crate::App>().invoke_action("group".into()),
        "fixture.power" => app.global::<crate::App>().invoke_action("power".into()),
        "fixture.aim" => app.global::<crate::App>().invoke_action("aim".into()),
        "fixture.drop" => app.global::<crate::App>().invoke_action("drop".into()),
        "fixture.number" => app.global::<crate::App>().invoke_action("number".into()),
        "dmx.readdress" => app.global::<crate::App>().invoke_action("readdress".into()),
        "tool.translate" => app.global::<crate::App>().set_tool(0),
        "tool.rotate" => app.global::<crate::App>().set_tool(1),
        "snap.toggle" => app.global::<crate::App>().set_snap(!app.global::<crate::App>().get_snap()),
        "view.persp" | "view.front" | "view.top" | "view.side" => app.global::<crate::App>().invoke_action(format!("view:{}", &id[5..]).into()),
        "view.fullscreen" => app.global::<crate::App>().invoke_action("fullscreen".into()),
        "menu.focus" => {
            let g = app.global::<crate::App>();
            g.set_menu_key(g.get_menu_key() + 1);
        }
        "view.fullview" => app.global::<crate::App>().set_full_view(!app.global::<crate::App>().get_full_view()),
        "view.help" => app.global::<crate::App>().set_show_help(!app.global::<crate::App>().get_show_help()),
        "view.fps" => app.global::<crate::App>().set_show_fps(!app.global::<crate::App>().get_show_fps()),
        "view.frame" => app.global::<crate::App>().invoke_action("frame".into()),
        "view.camera" => app.global::<crate::App>().invoke_action("camera".into()),
        "view.highlight" => app.global::<crate::App>().invoke_set_flag("highlight".into(), !app.global::<crate::App>().get_highlight()),
        "view.reset" => app.global::<crate::App>().invoke_reset_view(),
        "view.plot" => app.global::<crate::App>().invoke_action("toggle-plot".into()),
        "plot.panel" => app.global::<crate::App>().invoke_action("plot-panel".into()),
        "plot.print" => app.global::<crate::App>().invoke_plot_export("print".into()),
        "plot.png" | "plot.svg" | "plot.equipment" | "plot.patch" | "plot.mvr" => app.global::<crate::App>().invoke_plot_export(id.trim_start_matches("plot.").into()),
        "dmx.sequence" => app.global::<crate::App>().invoke_action("readdress-selection".into()),
        "labels.toggle" => app.global::<crate::App>().invoke_set_flag("labels".into(), !app.global::<crate::App>().get_show_labels()),
        "labels.patch" => app.global::<crate::App>().invoke_set_flag("labelPatch".into(), !app.global::<crate::App>().get_show_patch()),
        "env.floor" => app.global::<crate::App>().invoke_set_flag("floor".into(), !app.global::<crate::App>().get_show_floor()),
        "env.cyc" => app.global::<crate::App>().invoke_set_flag("cyc".into(), !app.global::<crate::App>().get_show_cyc()),
        "env.stands" => app.global::<crate::App>().invoke_set_flag("stands".into(), !app.global::<crate::App>().get_show_stands()),
        "view.bloom" => app.global::<crate::App>().invoke_set_flag("bloom".into(), !app.global::<crate::App>().get_show_bloom()),
        "view.blackout" => app.global::<crate::App>().invoke_action("blackout".into()),
        "panel.left" => app.global::<crate::App>().set_show_left(!app.global::<crate::App>().get_show_left()),
        "panel.right" => app.global::<crate::App>().set_show_right(!app.global::<crate::App>().get_show_right()),
        "panel.bottom" => app.global::<crate::App>().set_show_console(!app.global::<crate::App>().get_show_console()),
        "drawer.dmx" | "drawer.log" | "drawer.network" | "drawer.bluetooth" | "drawer.assistant" | "midi.open" => {
            let g = app.global::<crate::App>();
            let tab = match id {
                "drawer.dmx" => 1,
                "drawer.log" => 6,
                "drawer.network" => 7,
                "midi.open" => 2,
                "drawer.assistant" => 4,
                _ => 5,
            };
            g.set_show_console(true);
            g.set_bottom_tab(tab);
            match tab {
                2 => g.invoke_midi("show".into(), SharedString::new()),
                4 => g.invoke_ai("show".into(), SharedString::new()),
                5 => g.invoke_ble("show".into(), SharedString::new()),
                _ => {}
            }
        }
        "library.search" => {
            let g = app.global::<crate::App>();
            g.set_show_left(true);
            g.set_focus_search(g.get_focus_search() + 1);
        }
        "library.lights" | "library.stands" | "library.grip" | "library.decor" => {
            let tab = ["library.lights", "library.stands", "library.grip", "library.decor"].iter().position(|x| *x == id).unwrap_or(0);
            app.global::<crate::App>().set_show_left(true);
            app.global::<crate::App>().invoke_lib_action("tab".into(), tab.to_string().into());
        }
        "remote.toggle" => app.global::<crate::App>().set_remote_open(!app.global::<crate::App>().get_remote_open()),
        "console.full" => app.global::<crate::App>().invoke_console_action("full".into()),
        "console.black" => app.global::<crate::App>().invoke_console_action("black".into()),
        "console.releaseAll" => app.global::<crate::App>().invoke_console_action("release-all".into()),
        "drawer.console" => app.global::<crate::App>().set_show_console(!app.global::<crate::App>().get_show_console()),
        "console.release" => app.global::<crate::App>().invoke_console_release(),
        "ai.voice" => app.global::<crate::App>().invoke_ai("voice-down".into(), SharedString::new()),
        "ai.drawing" => app.global::<crate::App>().invoke_ai("drawing".into(), SharedString::new()),
        "shortcuts.open" => app.global::<crate::App>().invoke_action("shortcuts".into()),
        "app.about" => app.global::<crate::App>().set_about_open(true),
        "app.support" => app.global::<crate::App>().set_support_open(true),
        "app.update" => {
            app.global::<crate::App>().set_about_open(true);
            app.global::<crate::App>().invoke_about_check();
        }
        "app.news" => app.global::<crate::App>().invoke_update_action("news".into()),
        "app.source" => app.global::<crate::App>().invoke_open_url("https://github.com/22eme-Arkane/ARKLIGHT".into()),
        "app.issue" => app.global::<crate::App>().invoke_open_url("https://github.com/22eme-Arkane/ARKLIGHT/issues/new".into()),
        "app.quit" => app.global::<crate::App>().invoke_quit(),
        _ => return false,
    }
    true
}
