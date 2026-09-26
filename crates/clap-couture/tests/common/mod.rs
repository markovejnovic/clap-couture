//! A `Backend` that answers from a script and records what it was asked.

use core::cell::RefCell;
use std::io;

use clap_couture::interactive::{Backend, ConfirmPrompt, PromptError, SelectPrompt, TextPrompt};

/// One prompt the fake was shown.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Asked {
    pub(crate) default: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) kind: Kind,
    pub(crate) options: Vec<String>,
    pub(crate) question: String,
}

/// Which prompt method was called.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Confirm,
    Select,
    Text,
}

/// One scripted reply, consumed in order.
pub(crate) enum Reply {
    Cancel,
    Confirm(bool),
    Select(Option<&'static str>),
    Text(Option<&'static str>),
}

pub(crate) struct ScriptedBackend {
    asked: RefCell<Vec<Asked>>,
    available: bool,
    /// The script, last reply first, so the next one pops off the end.
    replies: RefCell<Vec<Reply>>,
}

impl ScriptedBackend {
    /// Everything asked so far, oldest first.
    pub(crate) fn asked(&self) -> Vec<Asked> {
        self.asked.take()
    }

    pub(crate) fn new(replies: impl IntoIterator<Item = Reply>) -> Self {
        Self {
            asked: RefCell::new(Vec::new()),
            available: true,
            replies: RefCell::new(
                replies.into_iter().collect::<Vec<_>>().into_iter().rev().collect(),
            ),
        }
    }

    fn next(&self, asked: Asked) -> Result<Reply, PromptError> {
        self.asked.borrow_mut().push(asked);
        self.replies
            .borrow_mut()
            .pop()
            .ok_or_else(|| PromptError::Io(io::Error::other("the script ran out of replies")))
    }

    /// A backend with no terminal to ask on.
    pub(crate) fn unavailable() -> Self {
        Self { available: false, ..Self::new([]) }
    }
}

impl Backend for ScriptedBackend {
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        let asked = Asked {
            default: Some(prompt.default.to_string()),
            error: prompt.error.map(str::to_owned),
            kind: Kind::Confirm,
            options: Vec::new(),
            question: prompt.question.to_owned(),
        };
        match self.next(asked)? {
            Reply::Confirm(yes) => Ok(yes),
            Reply::Cancel => Err(PromptError::Cancelled),
            Reply::Select(_) | Reply::Text(_) => Err(wrong_reply()),
        }
    }

    fn is_available(&self) -> bool {
        self.available
    }

    fn select(&self, prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
        let asked = Asked {
            default: prompt.default.map(str::to_owned),
            error: prompt.error.map(str::to_owned),
            kind: Kind::Select,
            options: prompt.options.iter().map(|value| value.get_name().to_owned()).collect(),
            question: prompt.question.to_owned(),
        };
        match self.next(asked)? {
            Reply::Select(name) => Ok(name.map(str::to_owned)),
            Reply::Cancel => Err(PromptError::Cancelled),
            Reply::Confirm(_) | Reply::Text(_) => Err(wrong_reply()),
        }
    }

    fn text(&self, prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
        let asked = Asked {
            default: prompt.default.map(str::to_owned),
            error: prompt.error.map(str::to_owned),
            kind: Kind::Text,
            options: Vec::new(),
            question: prompt.question.to_owned(),
        };
        match self.next(asked)? {
            Reply::Text(answer) => Ok(answer.map(str::to_owned)),
            Reply::Cancel => Err(PromptError::Cancelled),
            Reply::Confirm(_) | Reply::Select(_) => Err(wrong_reply()),
        }
    }
}

fn wrong_reply() -> PromptError {
    PromptError::Io(io::Error::other("the script's next reply is for another kind of prompt"))
}
