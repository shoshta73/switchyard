use switchyard_core::{Model, Provider};

use crate::provider;

pub(crate) enum Outcome {
    Ignored,
    Handled,
    OpenProviderMenu,
    OpenModelMenu,
    Exit,
}

pub(crate) struct Context<'a> {
    pub(crate) provider: &'a mut Provider,
    pub(crate) model: &'a mut Model,
    diagnostics: Vec<String>,
}

impl<'a> Context<'a> {
    pub(crate) fn new(provider: &'a mut Provider, model: &'a mut Model) -> Self {
        Self {
            provider,
            model,
            diagnostics: Vec::new(),
        }
    }

    pub(crate) fn push_diagnostic(&mut self, content: impl Into<String>) {
        self.diagnostics.push(content.into());
    }

    pub(crate) fn take_diagnostics(self) -> Vec<String> {
        self.diagnostics
    }
}

trait Command {
    fn name(&self) -> &'static str;
    fn execute(&self, context: &mut Context<'_>, args: &str) -> Outcome;
}

const COMMANDS: &[&dyn Command] = &[&ExitCommand, &HelpCommand, &ProviderCommand, &ModelCommand];

pub(crate) fn complete(prompt: &str) -> Option<String> {
    let command = prompt.strip_prefix('/')?;
    if command.is_empty() || command.chars().any(char::is_whitespace) {
        return None;
    }

    let mut matches = COMMANDS
        .iter()
        .map(|command| command.name())
        .filter(|name| name.starts_with(command));

    let name = matches.next()?;
    matches.next().is_none().then(|| format!("/{name}"))
}

pub(crate) fn handle(context: &mut Context<'_>, prompt: &str) -> Outcome {
    let Some(command) = prompt.strip_prefix('/') else {
        return Outcome::Ignored;
    };
    let Some((name, args)) = command
        .split_once(char::is_whitespace)
        .map(|(name, args)| (name, args.trim()))
        .or_else(|| (!command.is_empty()).then_some((command, "")))
    else {
        return Outcome::Ignored;
    };

    COMMANDS
        .iter()
        .find(|command| command.name() == name)
        .map_or(Outcome::Ignored, |command| command.execute(context, args))
}

struct ExitCommand;

impl Command for ExitCommand {
    fn name(&self) -> &'static str {
        "exit"
    }

    fn execute(&self, _context: &mut Context<'_>, _args: &str) -> Outcome {
        Outcome::Exit
    }
}

struct HelpCommand;

impl Command for HelpCommand {
    fn name(&self) -> &'static str {
        "help"
    }

    fn execute(&self, context: &mut Context<'_>, _args: &str) -> Outcome {
        context.push_diagnostic(
            "Available commands:\n/help - Show this help.\n/provider [ollama|llama.cpp] - Choose a provider.\n/model [name] - Choose or set a model.\n/exit - Exit Switchyard.",
        );
        Outcome::Handled
    }
}

struct ProviderCommand;

impl Command for ProviderCommand {
    fn name(&self) -> &'static str {
        "provider"
    }

    fn execute(&self, context: &mut Context<'_>, args: &str) -> Outcome {
        if args.is_empty() {
            return Outcome::OpenProviderMenu;
        }

        if !matches!(
            args.to_ascii_lowercase().as_str(),
            "ollama" | "llama.cpp" | "llamacpp" | "llama-cpp"
        ) {
            context.push_diagnostic(format!(
                "Unknown provider: {args}. Usage: /provider ollama|llama.cpp"
            ));
            return Outcome::Handled;
        }

        let local_provider = provider::LocalProvider::from_name(args);
        *context.provider = Provider::from(local_provider.name());
        *context.model = provider::LocalProvider::default_model_for(local_provider.name());
        context.push_diagnostic(format!(
            "Provider set to {}. Model set to {}.",
            context.provider.name, context.model.name
        ));
        Outcome::Handled
    }
}

struct ModelCommand;

impl Command for ModelCommand {
    fn name(&self) -> &'static str {
        "model"
    }

    fn execute(&self, context: &mut Context<'_>, args: &str) -> Outcome {
        if args.is_empty() {
            return Outcome::OpenModelMenu;
        }

        let value = if context.provider.name == "llama.cpp"
            && args.to_ascii_lowercase().ends_with(".gguf")
        {
            args[..args.len() - ".gguf".len()].to_string()
        } else {
            args.to_string()
        };
        *context.model = value.into();
        context.push_diagnostic(format!("Model set to {}.", context.model.name));
        Outcome::Handled
    }
}

#[cfg(test)]
mod tests {
    use switchyard_core::{Model, Provider};

    use super::{Context, Outcome, complete, handle};

    fn command_context<'a>(provider: &'a mut Provider, model: &'a mut Model) -> Context<'a> {
        Context::new(provider, model)
    }

    #[test]
    fn ignores_chat_prompt() {
        let mut provider = Provider::from("Ollama");
        let mut model = Model::from("llama3.2");
        let mut context = command_context(&mut provider, &mut model);

        assert!(matches!(handle(&mut context, "hello"), Outcome::Ignored));
    }

    #[test]
    fn completes_unique_command_prefix() {
        assert_eq!(complete("/pro"), Some("/provider".to_string()));
        assert_eq!(complete("/m"), Some("/model".to_string()));
        assert_eq!(complete("/e"), Some("/exit".to_string()));
        assert_eq!(complete("/h"), Some("/help".to_string()));
    }

    #[test]
    fn skips_command_completion_for_non_command_or_argument_input() {
        assert_eq!(complete("hello"), None);
        assert_eq!(complete("/"), None);
        assert_eq!(complete("/provider o"), None);
    }

    #[test]
    fn exits() {
        let mut provider = Provider::from("Ollama");
        let mut model = Model::from("llama3.2");
        let mut context = command_context(&mut provider, &mut model);

        assert!(matches!(handle(&mut context, "/exit"), Outcome::Exit));
    }

    #[test]
    fn shows_help() {
        let mut provider = Provider::from("Ollama");
        let mut model = Model::from("llama3.2");
        let mut context = command_context(&mut provider, &mut model);

        assert!(matches!(handle(&mut context, "/help"), Outcome::Handled));

        let diagnostics = context.take_diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].contains("Available commands:"));
        assert!(diagnostics[0].contains("/provider [ollama|llama.cpp]"));
        assert!(diagnostics[0].contains("/model [name]"));
        assert!(diagnostics[0].contains("/exit"));
    }

    #[test]
    fn sets_llama_cpp_provider_and_default_model() {
        let mut provider = Provider::from("Ollama");
        let mut model = Model::from("llama3.2");
        let mut context = command_context(&mut provider, &mut model);

        assert!(matches!(
            handle(&mut context, "/provider llama.cpp"),
            Outcome::Handled
        ));

        assert_eq!(context.provider.name, "llama.cpp");
        assert_eq!(context.model.name, "local-model");
    }

    #[test]
    fn opens_provider_menu_without_args() {
        let mut provider = Provider::from("Ollama");
        let mut model = Model::from("llama3.2");
        let mut context = command_context(&mut provider, &mut model);

        assert!(matches!(
            handle(&mut context, "/provider"),
            Outcome::OpenProviderMenu
        ));
    }

    #[test]
    fn opens_model_menu_without_args() {
        let mut provider = Provider::from("Ollama");
        let mut model = Model::from("llama3.2");
        let mut context = command_context(&mut provider, &mut model);

        assert!(matches!(
            handle(&mut context, "/model"),
            Outcome::OpenModelMenu
        ));
    }

    #[test]
    fn strips_llama_cpp_model_extension() {
        let mut provider = Provider::from("llama.cpp");
        let mut model = Model::from("local-model");
        let mut context = command_context(&mut provider, &mut model);

        assert!(matches!(
            handle(&mut context, "/model qwen.gguf"),
            Outcome::Handled
        ));

        assert_eq!(context.model.name, "qwen");
    }
}
