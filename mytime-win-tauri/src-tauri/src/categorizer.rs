//! Heuristic categorizer for MyTime
//!
//! Categorizes window titles into predefined categories based on pattern matching.
//! Categories: entertainment, development, productivity, communication, unknown

use crate::models::{Category, Label, LabelSource};
use crate::utils::now_ms;

/// Categorize a window based on app name and title using heuristics
pub fn categorize_heuristic(app_name: &str, window_title: &str) -> Category {
    let title_lower = window_title.to_lowercase();
    let app_lower = app_name.to_lowercase();

    // Entertainment - check first as it's often the most specific
    if is_entertainment(&app_lower, &title_lower) {
        return Category::Entertainment;
    }

    // Development
    if is_development(&app_lower, &title_lower) {
        return Category::Development;
    }

    // Productivity
    if is_productivity(&app_lower, &title_lower) {
        return Category::Productivity;
    }

    // Communication
    if is_communication(&app_lower, &title_lower) {
        return Category::Communication;
    }

    Category::Unknown
}

/// Create a heuristic label for a title_hash
pub fn create_heuristic_label(title_hash: &str, app_name: &str, window_title: &str) -> Label {
    let category = categorize_heuristic(app_name, window_title);
    Label {
        title_hash: title_hash.to_string(),
        category: category.as_str().to_string(),
        source: LabelSource::Heuristic,
        confidence: None, // Heuristics don't have confidence scores
        updated_at: now_ms(),
    }
}

fn is_entertainment(app: &str, title: &str) -> bool {
    // Video streaming
    if matches_any(
        title,
        &[
            "youtube",
            "netflix",
            "hulu",
            "disney+",
            "prime video",
            "twitch",
            "bilibili",
            "vimeo",
            "dailymotion",
        ],
    ) {
        return true;
    }

    // Social media
    if matches_any(
        title,
        &[
            "reddit",
            "twitter",
            "x.com",
            "facebook",
            "instagram",
            "tiktok",
            "snapchat",
            "pinterest",
            "tumblr",
            "weibo",
            "linkedin feed", // LinkedIn browsing (not job search)
        ],
    ) {
        return true;
    }

    // Music
    if matches_any(title, &["spotify", "apple music", "soundcloud", "bandcamp"])
        || matches_any(app, &["spotify.exe", "music.ui.exe"])
    {
        return true;
    }

    // Gaming launchers
    if matches_any(
        app,
        &[
            "steam.exe",
            "epicgameslauncher.exe",
            "origin.exe",
            "battle.net.exe",
            "riotclientservices.exe",
            "eadesktop.exe",
            "goggalaxy.exe",
            "ubisoft",
            "playnite",
        ],
    ) {
        return true;
    }

    // Popular games
    if matches_any(
        app,
        &[
            "bf6.exe",
            "bf2042.exe",
            "battlefield",
            "cod.exe",
            "blackops",
            "modernwarfare",
            "warzone",
            "valorant.exe",
            "valorant-win64",
            "league of legends",
            "leagueclient",
            "csgo.exe",
            "cs2.exe",
            "counterstrike",
            "dota2.exe",
            "overwatch.exe",
            "fortnite",
            "fortniteclient",
            "minecraft",
            "gta5.exe",
            "gtav.exe",
            "playgtav",
            "rdr2.exe",
            "eldenring",
            "cyberpunk2077",
            "baldursgate3",
            "bg3.exe",
            "diablo",
            "pathofexile",
            "lostark",
            "newworld",
            "apex.exe",
            "r5apex",
            "pubg",
            "destiny2",
            "halo",
            "starfield",
        ],
    ) {
        return true;
    }

    // Gaming titles in browser
    if matches_any(title, &["steam", "epic games", "itch.io", "gog.com"]) {
        return true;
    }

    // News and general browsing (often entertainment)
    if matches_any(
        title,
        &[
            "cnn",
            "bbc",
            "nytimes",
            "wsj",
            "theguardian",
            "reuters",
            "buzzfeed",
            "9gag",
            "imgur",
            "giphy",
            "hacker news",
            "product hunt",
            "amazon prime video",
            "hbo max",
            "crunchyroll",
            "funimation",
            "plex",
            "jellyfin",
            "emby",
        ],
    ) {
        return true;
    }

    false
}

fn is_development(app: &str, title: &str) -> bool {
    // IDEs and editors
    if matches_any(
        app,
        &[
            "code.exe",
            "devenv.exe",
            "idea64.exe",
            "webstorm64.exe",
            "pycharm64.exe",
            "goland64.exe",
            "clion64.exe",
            "rider64.exe",
            "android studio",
            "xcode",
            "sublime_text.exe",
            "atom.exe",
            "notepad++.exe",
            "cursor.exe",
            "windsurf.exe",
            "zed.exe",
        ],
    ) {
        return true;
    }

    // Terminal
    if matches_any(
        app,
        &[
            "windowsterminal.exe",
            "cmd.exe",
            "powershell.exe",
            "pwsh.exe",
            "wt.exe",
            "conhost.exe",
            "alacritty.exe",
            "hyper.exe",
        ],
    ) {
        return true;
    }

    // Development sites
    if matches_any(
        title,
        &[
            "github",
            "gitlab",
            "bitbucket",
            "stackoverflow",
            "stack overflow",
            "localhost",
            "127.0.0.1",
            "developer.",
            "docs.rs",
            "crates.io",
            "npmjs.com",
            "pypi.org",
            "rubygems.org",
            "maven",
            "gradle",
            "docker",
            "kubernetes",
            "jenkins",
            "circleci",
            "travis",
            "aws console",
            "azure portal",
            "google cloud",
            "mdn web docs",
            "mozilla developer",
            "w3schools",
            "devdocs",
            "rust-lang.org",
            "python.org/doc",
            "go.dev",
            "nodejs.org",
            "learn.microsoft",
            "vercel",
            "netlify",
            "heroku",
            "railway",
            "supabase",
            "firebase",
            "mongodb",
            "postgresql",
            "redis",
            "graphql",
            "postman",
            "swagger",
            "openapi",
            "leetcode",
            "hackerrank",
            "codewars",
            "exercism",
        ],
    ) {
        return true;
    }

    // Code-related titles
    if matches_any(
        title,
        &[
            ".rs",
            ".py",
            ".js",
            ".ts",
            ".go",
            ".java",
            ".cpp",
            ".c",
            ".html",
            ".css",
            ".json",
            ".yaml",
            ".toml",
            ".md",
            "pull request",
            "merge request",
            "commit",
            "branch",
            "api",
            "debug",
            "error",
            "exception",
            "compile",
        ],
    ) {
        return true;
    }

    false
}

fn is_productivity(app: &str, title: &str) -> bool {
    // AI assistants
    if matches_any(
        title,
        &[
            "claude",
            "chatgpt",
            "copilot",
            "gemini",
            "perplexity",
            "bard",
            "bing chat",
        ],
    ) {
        return true;
    }

    // Note-taking and knowledge management
    if matches_any(
        title,
        &[
            "notion",
            "obsidian",
            "roam",
            "logseq",
            "evernote",
            "onenote",
            "bear",
            "craft",
            "remnote",
        ],
    ) || matches_any(app, &["notion.exe", "obsidian.exe", "onenote.exe"])
    {
        return true;
    }

    // Office apps
    if matches_any(
        app,
        &[
            "winword.exe",
            "excel.exe",
            "powerpnt.exe",
            "libreoffice",
            "openoffice",
        ],
    ) {
        return true;
    }

    // Google Workspace
    if matches_any(
        title,
        &[
            "docs.google",
            "sheets.google",
            "slides.google",
            "google docs",
            "google sheets",
            "google slides",
            "google drive",
            "google calendar",
        ],
    ) {
        return true;
    }

    // Academic/writing
    if matches_any(
        title,
        &[
            "overleaf",
            "latex",
            "grammarly",
            "hemingway",
            "scrivener",
            "ulysses",
        ],
    ) {
        return true;
    }

    // Project management
    if matches_any(
        title,
        &[
            "jira",
            "asana",
            "trello",
            "monday.com",
            "clickup",
            "basecamp",
            "linear",
            "todoist",
            "ticktick",
        ],
    ) {
        return true;
    }

    // Design tools (productive use)
    if matches_any(app, &["figma.exe", "sketch.exe"])
        || matches_any(title, &["figma", "canva", "miro", "whimsical"])
    {
        return true;
    }

    // Research and reference
    if matches_any(
        title,
        &[
            "wikipedia",
            "arxiv",
            "scholar.google",
            "researchgate",
            "medium.com",
            "dev.to",
            "hashnode",
            "substack",
            "pdf",
            "documentation",
            "manual",
            "guide",
            "tutorial",
        ],
    ) {
        return true;
    }

    // Finance and work tools
    if matches_any(
        title,
        &[
            "quickbooks",
            "xero",
            "freshbooks",
            "invoice",
            "salesforce",
            "hubspot",
            "zendesk",
            "intercom",
            "airtable",
            "coda",
            "smartsheet",
        ],
    ) {
        return true;
    }

    false
}

fn is_communication(app: &str, title: &str) -> bool {
    // Chat apps
    if matches_any(
        app,
        &[
            "slack.exe",
            "discord.exe",
            "teams.exe",
            "telegram.exe",
            "signal.exe",
            "wechat.exe",
            "weixin.exe",
            "whatsapp.exe",
            "zoom.exe",
            "skype.exe",
            "webex.exe",
            "dingtalk.exe",
            "feishu.exe",
            "lark.exe",
        ],
    ) {
        return true;
    }

    // Email
    if matches_any(
        app,
        &[
            "outlook.exe",
            "olk.exe",
            "thunderbird.exe",
            "mailspring.exe",
        ],
    ) {
        return true;
    }

    // Web-based communication
    if matches_any(
        title,
        &[
            "gmail",
            "outlook.com",
            "mail",
            "inbox",
            "slack",
            "discord",
            "teams",
            "zoom",
            "google meet",
            "microsoft teams",
        ],
    ) {
        return true;
    }

    // Video calls
    if matches_any(title, &["meeting", "call", "conference", "webinar"]) {
        return true;
    }

    false
}

/// Check if text contains any of the patterns
fn matches_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| text.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entertainment_youtube() {
        assert_eq!(
            categorize_heuristic("msedge.exe", "YouTube - Rust Tutorial"),
            Category::Entertainment
        );
    }

    #[test]
    fn test_development_vscode() {
        assert_eq!(
            categorize_heuristic("code.exe", "main.rs - mytime-windows"),
            Category::Development
        );
    }

    #[test]
    fn test_productivity_claude() {
        assert_eq!(
            categorize_heuristic("msedge.exe", "Claude"),
            Category::Productivity
        );
    }

    #[test]
    fn test_communication_slack() {
        assert_eq!(
            categorize_heuristic("slack.exe", "Slack | general | MyCompany"),
            Category::Communication
        );
    }

    #[test]
    fn test_unknown_generic() {
        assert_eq!(
            categorize_heuristic("random.exe", "Some Random Window"),
            Category::Unknown
        );
    }
}
