//! Step references: a later step of a plan takes a value from the reply of an earlier step, such
//! as the id of the track that `create_track` makes, which cannot be known when the plan is made.
//!
//! A reference names where the value goes (a JSON pointer into the step's arguments), the earlier
//! step, and where the value lies in that step's reply (a JSON pointer such as
//! `/result/track_id`). Until the step runs, its argument holds the reference's placeholder text,
//! alone or inside a longer string: `"{step 4: /result/device_id}:dynamics"` becomes
//! `"dev_7:dynamics"`. The arguments therefore keep the types the tool's input model asks for and
//! can be validated before the earlier step has run.
//!
//! References also cut a plan into stages: a stage ends before the first step that needs a value
//! from a step of the same stage, so each stage can be validated against the project as the
//! earlier stages left it.

use std::ops::Range;

use serde::Serialize;
use serde_json::Value;

use super::Step;

/// A value that a step takes from the reply of an earlier step of the same plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StepRef {
    /// Where the value goes: a JSON pointer into the step's arguments, such as `/track_id` or
    /// `/add_sends/0/target`. The string there holds [`StepRef::placeholder`] until the step runs.
    pub arg: String,
    /// The earlier step whose reply holds the value: its index in the plan's steps, from 0.
    pub step: usize,
    /// Where the value lies in that step's reply: a JSON pointer such as `/result/track_id`.
    /// The value must be a string.
    pub pointer: String,
}

impl StepRef {
    /// The value at `pointer` in the reply of step `step`, written into the argument at `arg`.
    #[must_use]
    pub fn new(arg: impl Into<String>, step: usize, pointer: impl Into<String>) -> Self {
        Self {
            arg: arg.into(),
            step,
            pointer: pointer.into(),
        }
    }

    /// The text that stands for the value in the step's arguments until the step runs:
    /// `{step <step>: <pointer>}`, for example `{step 0: /result/track_id}`.
    #[must_use]
    pub fn placeholder(&self) -> String {
        format!("{{step {}: {}}}", self.step, self.pointer)
    }
}

/// Why a step's references cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum RefError {
    /// The reference names the step itself or a later step.
    #[error("it refers to step {0}, which does not run before it")]
    NotEarlier(usize),
    /// A pointer is neither empty nor starts with `/`.
    #[error("{0:?} is not a JSON pointer")]
    NotPointer(String),
    /// The argument is not a string that holds the placeholder exactly once.
    #[error("its argument {arg} does not hold {placeholder} exactly once")]
    NoPlaceholder {
        /// The argument's pointer.
        arg: String,
        /// The placeholder it should hold.
        placeholder: String,
    },
    /// The earlier step's reply has no string where the reference points.
    #[error("the reply of step {step} has no text at {pointer}")]
    Missing {
        /// The earlier step.
        step: usize,
        /// The pointer into its reply.
        pointer: String,
    },
}

/// Checks the references of `step`, the step at `index` of its plan, without running anything:
/// each names an earlier step, both of its pointers are JSON pointers, and its argument holds
/// its placeholder exactly once.
pub(crate) fn check(step: &Step, index: usize) -> Result<(), RefError> {
    for reference in &step.refs {
        if reference.step >= index {
            return Err(RefError::NotEarlier(reference.step));
        }
        for pointer in [&reference.arg, &reference.pointer] {
            if !pointer.is_empty() && !pointer.starts_with('/') {
                return Err(RefError::NotPointer(pointer.clone()));
            }
        }
        let placeholder = reference.placeholder();
        let held = step
            .args
            .pointer(&reference.arg)
            .and_then(Value::as_str)
            .is_some_and(|text| text.matches(&placeholder).count() == 1);
        if !held {
            return Err(RefError::NoPlaceholder {
                arg: reference.arg.clone(),
                placeholder,
            });
        }
    }
    Ok(())
}

/// The arguments of `step` with each placeholder replaced by the value its reference names.
/// `reply` gives the reply of a step of the plan by its index (`None` when it has none).
pub(crate) fn resolve<'a>(
    step: &Step,
    reply: impl Fn(usize) -> Option<&'a Value>,
) -> Result<Value, RefError> {
    let mut args = step.args.clone();
    for reference in &step.refs {
        let value = reply(reference.step)
            .and_then(|reply| reply.pointer(&reference.pointer))
            .and_then(Value::as_str)
            .ok_or_else(|| RefError::Missing {
                step: reference.step,
                pointer: reference.pointer.clone(),
            })?;
        let placeholder = reference.placeholder();
        let text = args
            .pointer(&reference.arg)
            .and_then(Value::as_str)
            .filter(|text| text.matches(&placeholder).count() == 1)
            .map(|text| text.replacen(&placeholder, value, 1))
            .ok_or_else(|| RefError::NoPlaceholder {
                arg: reference.arg.clone(),
                placeholder: placeholder.clone(),
            })?;
        if let Some(slot) = args.pointer_mut(&reference.arg) {
            *slot = Value::String(text);
        }
    }
    Ok(args)
}

/// The stages of a plan, as ranges of step indices in plan order. A stage ends before the first
/// step that takes a value from a step of the same stage, so every reference of a stage names a
/// step of an earlier stage, and the first stage has none. Expects references that passed
/// [`check`]; an empty plan has no stages.
pub(crate) fn stages(steps: &[Step]) -> Vec<Range<usize>> {
    let mut stages = Vec::new();
    let mut start = 0;
    for (index, step) in steps.iter().enumerate() {
        if step.refs.iter().any(|reference| reference.step >= start) {
            stages.push(start..index);
            start = index;
        }
    }
    if start < steps.len() {
        stages.push(start..steps.len());
    }
    stages
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn step(tool: &str, args: Value, refs: Vec<StepRef>) -> Step {
        Step {
            tool: tool.to_owned(),
            args,
            description: tool.to_owned(),
            destructive: false,
            refs,
        }
    }

    fn track_ref(step: usize) -> StepRef {
        StepRef::new("/track_id", step, "/result/track_id")
    }

    #[test]
    fn the_placeholder_names_the_step_and_the_pointer() {
        assert_eq!(track_ref(4).placeholder(), "{step 4: /result/track_id}");
    }

    #[test]
    fn a_placeholder_is_replaced_whole_or_inside_a_longer_string() {
        let device = StepRef::new("/target", 1, "/result/device_id");
        let clip = step(
            "create_automation",
            json!({"track_id": "{step 0: /result/track_id}", "target": "{step 1: /result/device_id}:dynamics", "points": []}),
            vec![track_ref(0), device],
        );
        let replies = [
            json!({"ok": true, "result": {"track_id": "trk_9"}}),
            json!({"ok": true, "result": {"device_id": "dev_7"}}),
        ];
        let args = resolve(&clip, |index| replies.get(index)).expect("both values exist");
        assert_eq!(
            args,
            json!({"track_id": "trk_9", "target": "dev_7:dynamics", "points": []})
        );
    }

    #[test]
    fn a_nested_argument_is_reached_by_its_pointer() {
        let send = StepRef::new("/add_sends/0/target", 0, "/result/track_id");
        let routing = step(
            "configure_routing",
            json!({"track_id": "trk_1", "add_sends": [{"target": send.placeholder(), "level_db": -12.0}]}),
            vec![send],
        );
        let reply = json!({"result": {"track_id": "trk_hall"}});
        let args = resolve(&routing, |_| Some(&reply)).expect("the value exists");
        assert_eq!(args["add_sends"][0]["target"], "trk_hall");
        assert_eq!(args["track_id"], "trk_1");
    }

    #[test]
    fn a_reply_without_the_value_cannot_resolve() {
        let clip = step(
            "create_midi_clip",
            json!({"track_id": "{step 0: /result/track_id}"}),
            vec![track_ref(0)],
        );
        let missing = Err(RefError::Missing {
            step: 0,
            pointer: "/result/track_id".to_owned(),
        });
        assert_eq!(resolve(&clip, |_| None), missing);
        let empty = json!({"ok": true});
        assert_eq!(resolve(&clip, |_| Some(&empty)), missing);
        // Only a string can stand in an argument's text.
        let number = json!({"result": {"track_id": 7}});
        assert_eq!(resolve(&clip, |_| Some(&number)), missing);
    }

    #[test]
    fn check_accepts_a_reference_to_an_earlier_step() {
        let clip = step(
            "create_midi_clip",
            json!({"track_id": "{step 0: /result/track_id}"}),
            vec![track_ref(0)],
        );
        assert_eq!(check(&clip, 1), Ok(()));
        assert_eq!(check(&clip, 3), Ok(()));
    }

    #[test]
    fn check_refuses_a_reference_to_itself_or_a_later_step() {
        let clip = step(
            "create_midi_clip",
            json!({"track_id": "{step 2: /result/track_id}"}),
            vec![track_ref(2)],
        );
        assert_eq!(check(&clip, 2), Err(RefError::NotEarlier(2)));
        assert_eq!(check(&clip, 1), Err(RefError::NotEarlier(2)));
    }

    #[test]
    fn check_refuses_a_pointer_without_a_leading_slash() {
        let reference = StepRef::new("track_id", 0, "/result/track_id");
        let clip = step(
            "create_midi_clip",
            json!({"track_id": reference.placeholder()}),
            vec![reference],
        );
        assert_eq!(
            check(&clip, 1),
            Err(RefError::NotPointer("track_id".to_owned()))
        );
        let reference = StepRef::new("/track_id", 0, "result.track_id");
        let clip = step(
            "create_midi_clip",
            json!({"track_id": reference.placeholder()}),
            vec![reference],
        );
        assert_eq!(
            check(&clip, 1),
            Err(RefError::NotPointer("result.track_id".to_owned()))
        );
    }

    #[test]
    fn check_refuses_an_argument_without_its_placeholder() {
        let no_placeholder = RefError::NoPlaceholder {
            arg: "/track_id".to_owned(),
            placeholder: "{step 0: /result/track_id}".to_owned(),
        };
        for args in [
            json!({"track_id": "trk_1"}),
            json!({"name": "{step 0: /result/track_id}"}),
            json!({"track_id": ["{step 0: /result/track_id}"]}),
            json!({"track_id": "{step 0: /result/track_id}{step 0: /result/track_id}"}),
        ] {
            let clip = step("create_midi_clip", args.clone(), vec![track_ref(0)]);
            assert_eq!(check(&clip, 1), Err(no_placeholder.clone()), "{args}");
            let reply = json!({"result": {"track_id": "trk_9"}});
            assert_eq!(
                resolve(&clip, |_| Some(&reply)),
                Err(no_placeholder.clone()),
                "{args}"
            );
        }
    }

    #[test]
    fn a_stage_ends_before_a_step_that_needs_a_value_from_it() {
        let track = |name: &str| step("create_track", json!({"name": name}), Vec::new());
        let on = |tool: &str, of: usize| {
            step(
                tool,
                json!({"track_id": track_ref(of).placeholder()}),
                vec![track_ref(of)],
            )
        };
        let device = |of: usize| {
            let reference = StepRef::new("/target", of, "/result/device_id");
            step(
                "create_automation",
                json!({"target": format!("{}:dynamics", reference.placeholder())}),
                vec![reference],
            )
        };
        let steps = [
            track("Violins"),                                    // 0
            track("Horns"),                                      // 1
            on("set_instrument", 0),                             // 2: needs stage 0
            on("set_instrument", 1),                             // 3
            on("create_midi_clip", 0),                           // 4
            step("set_tempo", json!({"bpm": 90.0}), Vec::new()), // 5
            device(2),                                           // 6: needs 2, of its stage
            on("set_track_pan", 1),                              // 7: needs stage 0 only
        ];
        assert_eq!(stages(&steps), [0..2, 2..6, 6..8]);
    }

    #[test]
    fn a_plan_without_references_is_one_stage() {
        let steps = [
            step("set_tempo", json!({"bpm": 90.0}), Vec::new()),
            step("delete_track", json!({"track_id": "trk_1"}), Vec::new()),
        ];
        let found = stages(&steps);
        assert_eq!(found.len(), 1);
        assert_eq!(found.first(), Some(&(0..steps.len())));
        assert_eq!(stages(&[]), Vec::<Range<usize>>::new());
    }
}
