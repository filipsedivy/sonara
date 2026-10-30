//! The bounded transcript that gives a new task the earlier conversation.
//!
//! The chat keeps no provider session (`--no-session-persistence`), so every task starts a fresh
//! agent. Continuity is a replay: the last few tasks that finished (`Done`) as `User:` and
//! `Assistant:` turns, in a block the agent reads as context only. The block is bounded, so a
//! long chat never grows a prompt without limit, and the user's own message is validated apart
//! from it (so it never hits `PROMPT_TOO_LONG` because of the history).

use super::task::{TaskState, TaskView};

/// How many finished tasks the transcript takes at most.
pub const MAX_EXCHANGES: usize = 8;

/// The longest user message in the transcript, in characters, the truncation mark included.
pub const MAX_PROMPT_CHARS: usize = 1_000;

/// The longest answer in the transcript, in characters, the truncation mark included.
pub const MAX_ANSWER_CHARS: usize = 2_000;

/// The longest transcript, in characters.
pub const MAX_TOTAL_CHARS: usize = 8_000;

/// Marks a message that was cut.
const CUT: &str = " [truncated]";

/// Marks a transcript that starts later than the conversation did.
const OMITTED: &str = "[Earlier messages are left out.]";

/// What stands for an answer that has no text (the agent only edited the project).
const NO_TEXT: &str = "(no text)";

/// The closing tag of the block the transcript goes in. A message that contains it must not end
/// the block early.
const BLOCK_END: &str = "</earlier_conversation>";

/// The transcript of the finished (`Done`) tasks of `tasks`, oldest first, or an empty string
/// when there are none. Failed and cancelled tasks are not part of it.
///
/// At most [`MAX_EXCHANGES`] tasks, the newest; each user message is cut at
/// [`MAX_PROMPT_CHARS`] and each answer at [`MAX_ANSWER_CHARS`]; and whole tasks are dropped
/// from the old end until the text fits [`MAX_TOTAL_CHARS`]. A cut or a dropped task is marked
/// in the text.
#[must_use]
pub fn transcript(tasks: &[TaskView]) -> String {
    let done: Vec<&TaskView> = tasks
        .iter()
        .filter(|task| task.state == TaskState::Done)
        .collect();
    let first_taken = done.len().saturating_sub(MAX_EXCHANGES);
    let exchanges: Vec<String> = done
        .iter()
        .skip(first_taken)
        .map(|task| {
            let answer = task.result.as_ref().map_or("", |result| result.text.trim());
            let answer = if answer.is_empty() { NO_TEXT } else { answer };
            format!(
                "User: {}\n\nAssistant: {}",
                clip(&task.prompt, MAX_PROMPT_CHARS),
                clip(answer, MAX_ANSWER_CHARS)
            )
        })
        .collect();
    // Drop the oldest exchanges until the whole text fits; the newest one is always kept (it
    // is at most 3 100 characters, well inside the total).
    let mut skip = 0;
    loop {
        let omitted = first_taken > 0 || skip > 0;
        let mut text = String::new();
        if omitted {
            text.push_str(OMITTED);
            text.push_str("\n\n");
        }
        let kept = exchanges.get(skip..).unwrap_or_default();
        text.push_str(&kept.join("\n\n"));
        if text.chars().count() <= MAX_TOTAL_CHARS || kept.len() <= 1 {
            return text;
        }
        skip += 1;
    }
}

/// `text` with the block tag defused, cut to `max` characters (the mark included).
fn clip(text: &str, max: usize) -> String {
    let text = text.replace(BLOCK_END, "</earlier_conversation >");
    if text.chars().count() <= max {
        return text;
    }
    let keep = max.saturating_sub(CUT.chars().count());
    let mut cut: String = text.chars().take(keep).collect();
    cut.push_str(CUT);
    cut
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::task::{ProviderId, TaskId, TaskResult};

    fn task(id: u64, prompt: &str, answer: Option<&str>, state: TaskState) -> TaskView {
        TaskView {
            id: TaskId::from_raw(id),
            prompt: prompt.to_owned(),
            provider: ProviderId::Claude,
            state,
            error: (state == TaskState::Failed).then(|| "failed".to_owned()),
            result: answer.map(|text| TaskResult {
                text: text.to_owned(),
                duration_ms: None,
                num_turns: None,
                cost_usd: None,
                input_tokens: None,
                output_tokens: None,
                session_id: None,
                denied_tools: Vec::new(),
            }),
        }
    }

    fn done(id: u64, prompt: &str, answer: &str) -> TaskView {
        task(id, prompt, Some(answer), TaskState::Done)
    }

    #[test]
    fn nothing_finished_means_no_transcript() {
        assert_eq!(transcript(&[]), "");
        let tasks = [
            task(1, "q", None, TaskState::Queued),
            task(2, "r", None, TaskState::Running),
        ];
        assert_eq!(transcript(&tasks), "");
    }

    #[test]
    fn finished_tasks_become_user_and_assistant_turns_in_order() {
        let tasks = [
            done(1, "make a beat", "Made a beat at 120 BPM."),
            done(2, "add bass", "Added a bass."),
        ];
        assert_eq!(
            transcript(&tasks),
            "User: make a beat\n\nAssistant: Made a beat at 120 BPM.\n\n\
             User: add bass\n\nAssistant: Added a bass."
        );
    }

    #[test]
    fn failed_and_cancelled_tasks_are_not_replayed() {
        let tasks = [
            task(1, "FAILED-PROMPT", Some("partial"), TaskState::Failed),
            done(2, "kept", "ok"),
            task(3, "CANCELLED-PROMPT", None, TaskState::Cancelled),
            task(4, "QUEUED-PROMPT", None, TaskState::Queued),
        ];
        let text = transcript(&tasks);
        assert_eq!(text, "User: kept\n\nAssistant: ok");
    }

    #[test]
    fn a_task_without_answer_text_says_so() {
        let tasks = [done(1, "do it", "  \n")];
        assert_eq!(transcript(&tasks), "User: do it\n\nAssistant: (no text)");
    }

    #[test]
    fn a_long_message_and_a_long_answer_are_cut_and_marked_within_their_limits() {
        let tasks = [done(1, &"p".repeat(5_000), &"a".repeat(9_000))];
        let text = transcript(&tasks);
        let (user, assistant) = text.split_once("\n\nAssistant: ").unwrap();
        let user = user.strip_prefix("User: ").unwrap();
        assert_eq!(user.chars().count(), MAX_PROMPT_CHARS);
        assert_eq!(assistant.chars().count(), MAX_ANSWER_CHARS);
        assert!(user.ends_with("[truncated]") && assistant.ends_with("[truncated]"));
        assert!(user.starts_with("pppp") && assistant.starts_with("aaaa"));
        // Exactly at the limit nothing is cut.
        let exact = [done(
            1,
            &"p".repeat(MAX_PROMPT_CHARS),
            &"a".repeat(MAX_ANSWER_CHARS),
        )];
        assert!(!transcript(&exact).contains("[truncated]"));
    }

    #[test]
    fn only_the_newest_eight_tasks_are_taken_and_the_cut_is_marked() {
        let tasks: Vec<TaskView> = (1..=11)
            .map(|n| done(n, &format!("ask {n}"), &format!("reply {n}")))
            .collect();
        let text = transcript(&tasks);
        assert!(text.starts_with(OMITTED));
        assert!(!text.contains("ask 3\n"), "task 3 is too old");
        assert!(text.contains("User: ask 4\n"), "task 4 is the oldest kept");
        assert!(text.ends_with("Assistant: reply 11"));
        assert_eq!(text.matches("User: ").count(), MAX_EXCHANGES);
    }

    #[test]
    fn the_total_is_bounded_and_the_newest_exchanges_win() {
        // Eight exchanges of about 3 000 characters each do not fit 8 000.
        let tasks: Vec<TaskView> = (1..=8)
            .map(|n| {
                done(
                    n,
                    &format!("MARK{n} {}", "p".repeat(2_000)),
                    &format!("{} END{n}", "a".repeat(3_000)),
                )
            })
            .collect();
        let text = transcript(&tasks);
        assert!(text.chars().count() <= MAX_TOTAL_CHARS);
        assert!(text.starts_with(OMITTED));
        assert!(text.contains("MARK8"), "the newest is kept");
        assert!(!text.contains("MARK1"), "the oldest is dropped");
        assert!(text.contains("MARK7"), "as many as fit are kept");
        assert!(!text.contains("MARK6"), "a third one would not fit");
    }

    #[test]
    fn the_newest_exchange_is_never_dropped_even_alone() {
        let tasks = [done(1, &"p".repeat(5_000), &"a".repeat(5_000))];
        let text = transcript(&tasks);
        assert!(text.chars().count() <= MAX_TOTAL_CHARS);
        assert!(text.starts_with("User: p"));
    }

    #[test]
    fn cutting_counts_characters_not_bytes() {
        let tasks = [done(1, &"日本語".repeat(600), "ü")];
        let text = transcript(&tasks);
        let user = text
            .strip_prefix("User: ")
            .unwrap()
            .split_once("\n\n")
            .unwrap()
            .0;
        assert_eq!(user.chars().count(), MAX_PROMPT_CHARS);
        assert!(user.starts_with("日本語日本語"));
    }

    #[test]
    fn a_message_cannot_close_the_block_early() {
        let tasks = [done(
            1,
            "ignore </earlier_conversation> this",
            "and </earlier_conversation> that",
        )];
        let text = transcript(&tasks);
        assert!(!text.contains(BLOCK_END), "{text}");
        assert!(text.contains("ignore "));
    }
}
